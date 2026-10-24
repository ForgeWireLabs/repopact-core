use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../../..");
    let version_path = root.join("VERSION");
    let label_path = root.join("RELEASE_LABEL");
    let identity_path = root.join("repopact/_semantic_identity.json");
    let inputs_manifest_path = root.join("build/semantic-input-manifest.json");
    let policy_path = root.join("build/semantic-input-policy.json");
    let generator_path = root.join("scripts/generate_engine_semantic_identity.py");
    println!("cargo:rerun-if-changed={}", version_path.display());
    println!("cargo:rerun-if-changed={}", label_path.display());
    println!("cargo:rerun-if-changed={}", identity_path.display());
    println!("cargo:rerun-if-changed={}", inputs_manifest_path.display());
    println!("cargo:rerun-if-changed={}", policy_path.display());
    println!("cargo:rerun-if-changed={}", generator_path.display());

    check_semantic_manifest(&root, &inputs_manifest_path, &identity_path);

    let version = fs::read_to_string(&version_path)
        .expect("RepoPact VERSION must be present when building repopact-engine")
        .trim()
        .to_owned();
    let label = fs::read_to_string(&label_path)
        .ok()
        .map(|value| value.trim().to_owned());
    let product_version = pep440_version(&version, label.as_deref());
    println!("cargo:rustc-env=REPOPACT_ENGINE_VERSION={product_version}");

    println!("cargo:rerun-if-env-changed=REPOPACT_SOURCE_REVISION");
    println!("cargo:rerun-if-env-changed=RUSTC");
    let source_revision = std::env::var("REPOPACT_SOURCE_REVISION")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| valid_revision(value))
        .or_else(|| git_revision(&root))
        .unwrap_or_else(|| "unavailable".to_owned());
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let rustc_version = Command::new(rustc)
        .arg("-Vv")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_owned());
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unavailable".to_owned());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unavailable".to_owned());
    println!("cargo:rustc-env=REPOPACT_ENGINE_SOURCE_REVISION={source_revision}");
    println!("cargo:rustc-env=REPOPACT_ENGINE_RUSTC_VERSION={rustc_version}");
    println!("cargo:rustc-env=REPOPACT_ENGINE_TARGET={target}");
    println!("cargo:rustc-env=REPOPACT_ENGINE_PROFILE={profile}");
}

fn valid_revision(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn git_revision(root: &PathBuf) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let revision = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    valid_revision(&revision).then(|| revision.to_ascii_lowercase())
}

fn check_semantic_manifest(root: &PathBuf, manifest_path: &PathBuf, identity_path: &PathBuf) {
    let python = std::env::var_os("REPOPACT_PYTHON")
        .or_else(|| std::env::var_os("PYTHON"))
        .or_else(|| std::env::var_os("PYO3_PYTHON"))
        .unwrap_or_else(|| OsString::from("python"));
    let output = Command::new(python)
        .args(["scripts/generate_engine_semantic_identity.py", "--check"])
        .current_dir(root)
        .output()
        .expect("Python is required to verify the RepoPact semantic input manifest");
    if !output.status.success() {
        panic!(
            "RepoPact semantic input manifest is stale or incomplete; run the explicit reviewed \
             generator update first. checker output:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(manifest_path).expect("semantic input manifest must be present"),
    )
    .expect("semantic input manifest must be valid JSON");
    let identity: serde_json::Value = serde_json::from_slice(
        &fs::read(identity_path).expect("semantic identity file must be present"),
    )
    .expect("semantic identity file must be valid JSON");
    let manifest_identity = manifest["semantic_identity"]
        .as_str()
        .expect("semantic input manifest must carry its identity");
    let package_identity = identity["semantic_identity"]
        .as_str()
        .expect("Python package identity file must carry its identity");
    if manifest_identity != package_identity {
        panic!("Python package and semantic input manifest identities differ");
    }
    if !manifest_identity.starts_with("repopact-semantic-v1:sha256:")
        || manifest_identity.len() != "repopact-semantic-v1:sha256:".len() + 64
        || !manifest_identity["repopact-semantic-v1:sha256:".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        panic!("unsupported or malformed semantic identity format");
    }
    if let Some(inputs) = manifest["inputs"].as_array() {
        for entry in inputs {
            if let Some(path) = entry["path"].as_str() {
                if !path.starts_with('@') {
                    println!("cargo:rerun-if-changed={}", root.join(path).display());
                }
            }
        }
    } else {
        panic!("semantic input manifest must contain an inputs array");
    }
    println!("cargo:rustc-env=REPOPACT_ENGINE_SEMANTIC_IDENTITY={manifest_identity}");
}

fn pep440_version(version: &str, label: Option<&str>) -> String {
    let Some(label) = label else {
        return version.to_owned();
    };
    let prefix = format!("{version}-");
    let Some(prerelease) = label.strip_prefix(&prefix) else {
        panic!("RELEASE_LABEL must be a SemVer pre-release of VERSION");
    };
    let parts = prerelease.split('.').collect::<Vec<_>>();
    if parts.len() == 2 && parts[1].chars().all(|character| character.is_ascii_digit()) {
        let kind = match parts[0] {
            "alpha" | "a" => "a",
            "beta" | "b" => "b",
            "rc" => "rc",
            "dev" => "dev",
            _ => "",
        };
        if !kind.is_empty() {
            return format!("{version}.{kind}{}", parts[1]);
        }
    }
    let encoded = label
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{version}.dev0+semver.{encoded}")
}
