//! Structural source-to-documentation freshness checks (WI047, Decision 0076).
//!
//! These checks compare explicitly mapped file bytes with an adopter-reviewed
//! snapshot. They identify drift and never interpret prose or execute a
//! generator.

use std::fs;
use std::path::{Path, PathBuf};

use repopact_repository::{normalize_path, resolve_within_root, RepositorySnapshot};
use repopact_types::hex_digest;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::Validator;

const MAPPING_SCHEMA: &str = "documentation-mappings.schema.json";
const DEFAULT_MAPPING_PATH: &str = "governance/documentation-mappings.json";

impl Validator {
    pub(super) fn validate_documentation_mappings(&mut self) {
        let owners = self
            .index
            .owners
            .as_ref()
            .and_then(|record| record.value.as_ref().ok());
        let configured = owners
            .and_then(|value| value.get("documentation_mappings"))
            .cloned();
        let Some(config) = configured else {
            return;
        };

        let owners_path = self.repository.root().join("governance/owners.json");
        if !config.is_object() || config.get("enabled").and_then(Value::as_bool).is_none() {
            self.push(self.at(
                "documentation.mapping-config-invalid",
                "documentation_mappings must be an object with a boolean enabled field",
                &owners_path,
            ));
            return;
        }
        if config.get("enabled").and_then(Value::as_bool) != Some(true) {
            return;
        }
        let relative = config
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_MAPPING_PATH);
        let Some(path) = safe_repository_path(self.repository.root(), relative) else {
            self.push(self.at(
                "documentation.mapping-config-invalid",
                "documentation_mappings.path must be a repository-relative POSIX path without dot-segments",
                &owners_path,
            ));
            return;
        };
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.push(self.at(
                    "documentation.mappings-missing",
                    format!("unable to read enabled documentation mapping file: {error}"),
                    &path,
                ));
                return;
            }
        };
        let data: Value = match serde_json::from_slice(&bytes) {
            Ok(data) => data,
            Err(error) => {
                self.push(self.at(
                    "documentation.mappings-invalid-json",
                    format!("documentation mapping file is not valid JSON: {error}"),
                    &path,
                ));
                return;
            }
        };
        self.extend_schema(&data, MAPPING_SCHEMA, &path);
        let Some(mappings) = data.get("mappings").and_then(Value::as_array) else {
            return;
        };
        let mut seen_ids = std::collections::BTreeSet::new();
        for mapping in mappings {
            let Some(id) = mapping.get("id").and_then(Value::as_str) else {
                continue;
            };
            if !seen_ids.insert(id.to_owned()) {
                self.push(self.at(
                    "documentation.mappings-invalid",
                    format!("duplicate documentation mapping id '{id}'"),
                    &path,
                ));
                continue;
            }
            let Some(kind) = mapping.get("kind").and_then(Value::as_str) else {
                continue;
            };
            let Some(sources) = string_paths(mapping.get("source_paths")) else {
                continue;
            };
            let Some(documents) = string_paths(mapping.get("documentation_paths")) else {
                continue;
            };
            if sources.is_empty() || documents.is_empty() {
                continue;
            }
            let result = match (
                digest_paths(self.repository.root(), &sources),
                digest_paths(self.repository.root(), &documents),
            ) {
                (Ok(source_digest), Ok(documentation_digest)) => {
                    Some((source_digest, documentation_digest))
                }
                (Err(message), _) | (_, Err(message)) => {
                    self.push(self.at(
                        "documentation.mapping-path-invalid",
                        format!("mapping '{id}': {message}"),
                        &path,
                    ));
                    None
                }
            };
            let Some((source_digest, documentation_digest)) = result else {
                continue;
            };
            let snapshot = mapping.get("snapshot");
            let expected_source = snapshot
                .and_then(|value| value.get("source_digest"))
                .and_then(Value::as_str);
            let expected_docs = snapshot
                .and_then(|value| value.get("documentation_digest"))
                .and_then(Value::as_str);
            if expected_source == Some(source_digest.as_str())
                && expected_docs == Some(documentation_digest.as_str())
            {
                continue;
            }
            let (code, reason) = match (
                expected_source == Some(source_digest.as_str()),
                expected_docs == Some(documentation_digest.as_str()),
                kind,
            ) {
                (false, true, "generated") => (
                    "documentation.generated-stale",
                    "mapped source/generator changed while generated documentation stayed at its reviewed bytes",
                ),
                (false, true, _) => (
                    "documentation.mapping-stale",
                    "mapped source changed while related documentation stayed at its reviewed bytes",
                ),
                (true, false, _) => (
                    "documentation.mapping-review-required",
                    "mapped documentation changed and requires an explicit freshness review",
                ),
                _ => (
                    "documentation.mapping-review-required",
                    "mapped source and documentation bytes changed; reconcile the pair and refresh the reviewed snapshot",
                ),
            };
            self.push(self.at(
                code,
                format!("mapping '{id}': {reason}; use `repopact documentation snapshot {id}` after review/regeneration and merge the returned hashes into the mapping record"),
                &path,
            ));
        }
    }
}

pub(super) fn compute_documentation_snapshot(
    snapshot: &RepositorySnapshot,
    mapping_id: &str,
) -> Result<Value, String> {
    let root = snapshot.repository().root();
    let owners_path = root.join("governance/owners.json");
    let owners: Value = serde_json::from_slice(
        &fs::read(&owners_path)
            .map_err(|error| format!("unable to read governance/owners.json: {error}"))?,
    )
    .map_err(|error| format!("governance/owners.json is not valid JSON: {error}"))?;
    let config = owners.get("documentation_mappings").unwrap_or(&Value::Null);
    if config.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Err("documentation mappings are not enabled in governance/owners.json".to_owned());
    }
    let relative = config
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_MAPPING_PATH);
    let path = safe_repository_path(root, relative).ok_or_else(|| {
        "documentation_mappings.path is not a safe repository-relative path".to_owned()
    })?;
    let data: Value = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("unable to read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{} is not valid JSON: {error}", path.display()))?;
    let mappings = data
        .get("mappings")
        .and_then(Value::as_array)
        .ok_or_else(|| "documentation mapping file must contain a mappings array".to_owned())?;
    let mapping = mappings
        .iter()
        .find(|mapping| mapping.get("id").and_then(Value::as_str) == Some(mapping_id))
        .ok_or_else(|| format!("no documentation mapping with id '{mapping_id}'"))?;
    let sources = string_paths(mapping.get("source_paths"))
        .ok_or_else(|| format!("mapping '{mapping_id}' has invalid source_paths"))?;
    let documents = string_paths(mapping.get("documentation_paths"))
        .ok_or_else(|| format!("mapping '{mapping_id}' has invalid documentation_paths"))?;
    Ok(json!({
        "source_digest": digest_paths(root, &sources)?,
        "documentation_digest": digest_paths(root, &documents)?
    }))
}

fn string_paths(value: Option<&Value>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|entry| entry.as_str().map(str::to_owned))
        .collect()
}

fn safe_repository_path(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.contains('\\')
        || relative.contains(':')
        || relative.chars().any(char::is_control)
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    let resolved = resolve_within_root(root, relative)?;
    resolved
        .starts_with(normalize_path(root))
        .then_some(resolved)
}

fn digest_paths(root: &Path, paths: &[String]) -> Result<String, String> {
    let mut sorted = paths.to_vec();
    sorted.sort();
    let mut hasher = Sha256::new();
    for relative in sorted {
        let path = safe_repository_path(root, &relative)
            .ok_or_else(|| format!("'{relative}' is not a safe repository-relative path"))?;
        let metadata = fs::metadata(&path)
            .map_err(|error| format!("mapped file '{relative}' is unavailable: {error}"))?;
        if !metadata.is_file() {
            return Err(format!("mapped path '{relative}' is not a regular file"));
        }
        let bytes = fs::read(&path)
            .map_err(|error| format!("unable to read mapped file '{relative}': {error}"))?;
        hasher.update((relative.len() as u64).to_be_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    Ok(hex_digest(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use repopact_repository::Repository;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("repopact-doc-map-{name}-{suffix}"));
        fs::create_dir_all(root.join("governance")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(
            root.join("governance/owners.json"),
            r#"{"documentation_mappings":{"enabled":true,"path":"governance/documentation-mappings.json"}}"#,
        )
        .unwrap();
        fs::write(root.join("src/api.py"), "VERSION = 1\n").unwrap();
        fs::write(root.join("docs/api.md"), "Version 1.\n").unwrap();
        root
    }

    fn write_mapping(root: &Path, kind: &str, source: &str, documentation: &str) {
        let mapping = json!({
            "$schema": "documentation-mappings.schema.json",
            "version": 1,
            "mappings": [{
                "id": "api-contract",
                "kind": kind,
                "source_paths": [source],
                "documentation_paths": [documentation],
                "description": "Example mapping",
                "snapshot": {
                    "source_digest": digest_paths(root, &[source.to_owned()]).unwrap(),
                    "documentation_digest": digest_paths(root, &[documentation.to_owned()]).unwrap()
                }
            }]
        });
        fs::write(
            root.join("governance/documentation-mappings.json"),
            serde_json::to_vec_pretty(&mapping).unwrap(),
        )
        .unwrap();
    }

    fn findings(root: &Path) -> Vec<String> {
        let mut validator = Validator::new(Repository::open(root));
        validator.validate_documentation_mappings();
        validator
            .diagnostics
            .into_iter()
            .map(|item| item.code)
            .collect()
    }

    #[test]
    fn reviewed_mapping_accepts_matching_source_and_documentation_bytes() {
        let root = root("valid");
        write_mapping(&root, "manual", "src/api.py", "docs/api.md");
        assert!(findings(&root).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_only_change_is_blocking_for_manual_and_generated_mappings() {
        for (name, kind, expected) in [
            ("manual-stale", "manual", "documentation.mapping-stale"),
            (
                "generated-stale",
                "generated",
                "documentation.generated-stale",
            ),
        ] {
            let root = root(name);
            write_mapping(&root, kind, "src/api.py", "docs/api.md");
            fs::write(root.join("src/api.py"), "VERSION = 2\n").unwrap();
            assert!(findings(&root).iter().any(|code| code == expected));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn changed_pair_requires_review_then_new_snapshot_restores_validity() {
        let root = root("review-pair");
        write_mapping(&root, "manual", "src/api.py", "docs/api.md");
        fs::write(root.join("src/api.py"), "VERSION = 2\n").unwrap();
        fs::write(root.join("docs/api.md"), "Version 2.\n").unwrap();
        assert!(findings(&root)
            .iter()
            .any(|code| code == "documentation.mapping-review-required"));
        let repository = Repository::open(&root);
        let snapshot = repository.session().snapshot();
        let result = compute_documentation_snapshot(&snapshot, "api-contract").unwrap();
        let path = root.join("governance/documentation-mappings.json");
        let mut mapping: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        mapping["mappings"][0]["snapshot"] = result;
        fs::write(&path, serde_json::to_vec_pretty(&mapping).unwrap()).unwrap();
        assert!(findings(&root).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mapping_path_escape_is_rejected() {
        let root = root("path-escape");
        let mut mapping = json!({
            "$schema": "documentation-mappings.schema.json",
            "version": 1,
            "mappings": [{
                "id": "api-contract",
                "kind": "manual",
                "source_paths": ["../outside.py"],
                "documentation_paths": ["docs/api.md"],
                "description": "Example mapping",
                "snapshot": {"source_digest": "0".repeat(64), "documentation_digest": "0".repeat(64)}
            }]
        });
        mapping["mappings"][0]["snapshot"]["documentation_digest"] =
            digest_paths(&root, &["docs/api.md".to_owned()])
                .unwrap()
                .into();
        fs::write(
            root.join("governance/documentation-mappings.json"),
            serde_json::to_vec_pretty(&mapping).unwrap(),
        )
        .unwrap();
        assert!(findings(&root)
            .iter()
            .any(|code| code == "documentation.mapping-path-invalid"));
        fs::remove_dir_all(root).unwrap();
    }
}
