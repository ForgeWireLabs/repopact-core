//! Bounded repository-local persistence for ordinary mutation recovery.
//!
//! This module deliberately stores one active journal per working tree and
//! immutable compact receipts. It has no recovery or cleanup-on-read path.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use repopact_repository::path_string;
use repopact_types::RepositoryIdentity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{MutationRequest, MutationResult, PathOperation, PreimageEntry, Preimages};

pub(super) const STORE_VERSION: u32 = 1;
pub(super) const MAX_RECEIPTS: usize = 1024;
pub(super) const MAX_PREIMAGE_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_JOURNAL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 64 * 1024;
const MAX_PROJECTION_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const MAX_PREIMAGE_ENTRIES: usize = 10_000;
const ANCHOR: &str = "work/.repopact-transactions/ordinary-v1";
const JOURNAL_FILE: &str = "journal.json";
const RECEIPTS_DIR: &str = "receipts";
const TEMPORARY_COUNTER_START: u64 = 1;

static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(TEMPORARY_COUNTER_START);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Prepared,
    Applying,
    RollingBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct NativeRootIdentity {
    pub canonical_path: String,
    pub device: u64,
    pub file_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct StoredEntry {
    pub path: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct StoredPreimages {
    pub roots: Vec<String>,
    pub entries: Vec<StoredEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Journal {
    pub version: u32,
    pub request_key: String,
    pub payload_sha256: String,
    pub repository_identity: RepositoryIdentity,
    pub native_root: NativeRootIdentity,
    pub storage_identity: NativeRootIdentity,
    pub plan_token: String,
    pub request: MutationRequest,
    pub operations: Vec<PathOperation>,
    pub roots: Vec<String>,
    pub before: StoredPreimages,
    pub projected_after: StoredPreimages,
    pub step_sha256: Vec<String>,
    pub phase: Phase,
    pub next_operation: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Receipt {
    pub version: u32,
    pub request_key: String,
    pub payload_sha256: String,
    pub repository_identity: RepositoryIdentity,
    pub native_root: NativeRootIdentity,
    pub storage_identity: NativeRootIdentity,
    pub plan_token: String,
    pub result: MutationResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Envelope<T> {
    pub(super) payload: T,
    pub(super) sha256: String,
}

pub(super) struct OrdinaryTransactionStore {
    root: PathBuf,
    anchor: PathBuf,
    journal_path: PathBuf,
    receipts_path: PathBuf,
}

impl NativeRootIdentity {
    pub(super) fn capture(root: &Path) -> Result<Self, String> {
        let canonical = fs::canonicalize(root)
            .map_err(|error| format!("canonicalize working root: {error}"))?;
        let metadata = fs::metadata(&canonical)
            .map_err(|error| format!("inspect canonical working root: {error}"))?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err("working root is not a regular directory".to_owned());
        }
        #[cfg(unix)]
        let (device, file_id) = {
            use std::os::unix::fs::MetadataExt;
            (metadata.dev(), metadata.ino())
        };
        #[cfg(windows)]
        let (device, file_id) = { windows_directory_identity(&canonical)? };
        #[cfg(not(any(unix, windows)))]
        let (device, file_id) = {
            let _ = metadata;
            return Err("native working-root identity is unsupported on this platform".to_owned());
        };
        Ok(Self {
            canonical_path: lossless_path_string(&canonical)?,
            device,
            file_id,
        })
    }
}

impl StoredPreimages {
    pub(super) fn from_live(preimages: &Preimages) -> Result<Self, String> {
        if preimages.entries.len() > MAX_PREIMAGE_ENTRIES {
            return Err("transaction preimage exceeds the entry-count limit".to_owned());
        }
        let mut total = 0u64;
        let roots = preimages
            .roots
            .iter()
            .map(|path| lossless_path_string(path))
            .collect::<Result<Vec<_>, _>>()?;
        let mut entries = Vec::with_capacity(preimages.entries.len());
        for (path, entry) in &preimages.entries {
            let (kind, bytes_hex, sha256) = match entry {
                PreimageEntry::Directory => ("directory", None, None),
                PreimageEntry::File(bytes) => {
                    total = total.saturating_add(bytes.len() as u64);
                    if total > MAX_PREIMAGE_BYTES {
                        return Err("transaction preimage exceeds the 64 MiB byte limit".to_owned());
                    }
                    (
                        "file",
                        Some(hex_encode(bytes)),
                        Some(digest_hex(Sha256::digest(bytes))),
                    )
                }
            };
            entries.push(StoredEntry {
                path: lossless_path_string(path)?,
                kind: kind.to_owned(),
                bytes_hex,
                sha256,
            });
        }
        Ok(Self { roots, entries })
    }

    pub(super) fn to_live(&self) -> Result<Preimages, String> {
        if self.entries.len() > MAX_PREIMAGE_ENTRIES {
            return Err("transaction preimage exceeds the entry-count limit".to_owned());
        }
        let roots = self
            .roots
            .iter()
            .map(|path| parse_relative(path))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let mut entries = BTreeMap::new();
        let mut total = 0u64;
        for stored in &self.entries {
            let path = parse_relative(&stored.path)?;
            let entry = match stored.kind.as_str() {
                "directory" if stored.bytes_hex.is_none() && stored.sha256.is_none() => {
                    PreimageEntry::Directory
                }
                "file" => {
                    let encoded = stored
                        .bytes_hex
                        .as_deref()
                        .ok_or_else(|| "transaction file preimage omitted its bytes".to_owned())?;
                    let bytes = hex_decode(encoded)?;
                    total = total.saturating_add(bytes.len() as u64);
                    if total > MAX_PREIMAGE_BYTES {
                        return Err("transaction preimage exceeds the 64 MiB byte limit".to_owned());
                    }
                    if stored.sha256.as_deref() != Some(digest_hex(Sha256::digest(&bytes)).as_str())
                    {
                        return Err(format!(
                            "transaction preimage '{}' failed its digest check",
                            stored.path
                        ));
                    }
                    PreimageEntry::File(bytes)
                }
                _ => return Err("transaction preimage has an invalid entry kind".to_owned()),
            };
            if entries.insert(path, entry).is_some() {
                return Err("transaction preimage contains duplicate paths".to_owned());
            }
        }
        Ok(Preimages { roots, entries })
    }
}

impl OrdinaryTransactionStore {
    pub(super) fn open(root: &Path) -> Result<Self, String> {
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| format!("canonicalize transaction repository root: {error}"))?;
        let root_metadata = fs::symlink_metadata(&canonical_root)
            .map_err(|error| format!("inspect transaction repository root: {error}"))?;
        if !root_metadata.is_dir() || is_reparse_point(&root_metadata) {
            return Err("transaction repository root is not a regular directory".to_owned());
        }
        let _ = NativeRootIdentity::capture(&canonical_root)?;
        let anchor = ensure_anchor(&canonical_root)?;
        let receipts_path = anchor.join(RECEIPTS_DIR);
        ensure_private_directory(&receipts_path)?;
        let journal_path = anchor.join(JOURNAL_FILE);
        Ok(Self {
            root: canonical_root,
            anchor,
            journal_path,
            receipts_path,
        })
    }

    pub(super) fn open_read_only(root: &Path) -> Result<Option<Self>, String> {
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| format!("canonicalize transaction repository root: {error}"))?;
        let anchor = canonical_root.join(ANCHOR.replace('/', std::path::MAIN_SEPARATOR_STR));
        if !validate_anchor_chain(&canonical_root)? {
            return Ok(None);
        }
        #[cfg(windows)]
        check_private_directory(&canonical_root.join("work/.repopact-transactions"))?;
        check_private_directory(&anchor)?;
        let receipts_path = anchor.join(RECEIPTS_DIR);
        match fs::symlink_metadata(&receipts_path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect terminal receipt directory: {error}")),
            Ok(metadata) if !metadata.is_dir() || is_reparse_point(&metadata) => {
                return Err("terminal receipt path is not a regular directory".to_owned())
            }
            Ok(_) => check_private_directory(&receipts_path)?,
        }
        Ok(Some(Self {
            root: canonical_root,
            journal_path: anchor.join(JOURNAL_FILE),
            anchor,
            receipts_path,
        }))
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn anchor(&self) -> &Path {
        &self.anchor
    }

    pub(super) fn has_unknown_artifacts(&self) -> Result<Vec<String>, String> {
        let mut unknown = Vec::new();
        for (directory, allowed) in [
            (&self.anchor, JOURNAL_FILE),
            (&self.receipts_path, "*.json"),
        ] {
            let entries = match fs::read_dir(directory) {
                Ok(entries) => entries,
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        && directory == &self.receipts_path =>
                {
                    continue
                }
                Err(error) => return Err(format!("enumerate transaction state: {error}")),
            };
            for entry in entries {
                let entry =
                    entry.map_err(|error| format!("enumerate transaction state: {error}"))?;
                let name = entry.file_name().to_string_lossy().to_string();
                if directory == &self.anchor {
                    if matches!(
                        name.as_str(),
                        JOURNAL_FILE | RECEIPTS_DIR | "repository.lock"
                    ) || name.starts_with(".journal-")
                    {
                        if name.starts_with(".journal-") {
                            unknown.push(path_string(&entry.path()));
                        }
                        continue;
                    }
                } else if allowed == "*.json" && name.ends_with(".json") && name.len() == 69 {
                    continue;
                }
                unknown.push(path_string(&entry.path()));
            }
        }
        Ok(unknown)
    }

    pub(super) fn load_journal(&self) -> Result<Option<Journal>, String> {
        let bytes = match read_regular_file(&self.journal_path, MAX_JOURNAL_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error == "missing" => return Ok(None),
            Err(error) => return Err(error),
        };
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("transaction journal is corrupt: {error}"))?;
        let payload_value = value
            .get("payload")
            .ok_or_else(|| "transaction journal is corrupt: payload is missing".to_owned())?;
        let operations = payload_value
            .get("operations")
            .and_then(Value::as_array)
            .ok_or_else(|| "transaction journal operation list is malformed".to_owned())?;
        if operations.is_empty() || operations.len() > 3 {
            return Err(
                "transaction journal operation count exceeds the typed transaction bound"
                    .to_owned(),
            );
        }
        let before_entries = payload_value
            .get("before")
            .and_then(|before| before.get("entries"))
            .and_then(Value::as_array)
            .ok_or_else(|| "transaction journal preimage list is malformed".to_owned())?;
        if before_entries.len() > MAX_PREIMAGE_ENTRIES {
            return Err("transaction journal preimage exceeds the entry-count limit".to_owned());
        }
        let before_bytes = before_entries.iter().try_fold(0u64, |total, entry| {
            let encoded = entry
                .get("bytes_hex")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let path_bytes = entry
                .get("path")
                .and_then(Value::as_str)
                .map_or(0, |path| path.len() as u64);
            total
                .checked_add((encoded.len() / 2) as u64 + path_bytes.saturating_mul(2) + 128)
                .ok_or_else(|| "transaction journal preimage size overflow".to_owned())
        })?;
        let root_overhead = payload_value
            .get("before")
            .and_then(|before| before.get("roots"))
            .and_then(Value::as_array)
            .map(|roots| {
                roots
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|path| path.len() as u64 + 64)
                    .sum::<u64>()
            })
            .unwrap_or_default();
        let write_bytes = operations.iter().try_fold(0u64, |total, operation| {
            let content = operation
                .get("Write")
                .and_then(|write| write.get("content"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let path_len = operation
                .get("Write")
                .and_then(|write| write.get("path"))
                .and_then(Value::as_str)
                .map_or(0, |path| path.len() as u64);
            let move_paths = operation.get("Move").map_or(0, |movement| {
                [movement.get("from"), movement.get("to")]
                    .into_iter()
                    .filter_map(|value| value.and_then(Value::as_str))
                    .map(|path| path.len() as u64 + 128)
                    .sum::<u64>()
            });
            total
                .checked_add(content.len() as u64 + path_len.saturating_mul(2) + 256 + move_paths)
                .ok_or_else(|| "transaction journal projected size overflow".to_owned())
        })?;
        let move_delta = operations.iter().try_fold(0u64, |total, operation| {
            let extra = operation.get("Move").map_or(0, |movement| {
                let from = movement
                    .get("from")
                    .and_then(Value::as_str)
                    .map_or(0, str::len);
                let to = movement
                    .get("to")
                    .and_then(Value::as_str)
                    .map_or(0, str::len);
                (to.saturating_sub(from) as u64)
                    .saturating_mul(before_entries.len() as u64)
                    .saturating_mul(2)
            });
            total
                .checked_add(extra)
                .ok_or_else(|| "transaction journal move projection size overflow".to_owned())
        })?;
        if before_bytes
            .checked_add(root_overhead)
            .and_then(|sum| sum.checked_add(move_delta))
            .and_then(|sum| sum.checked_add(write_bytes))
            .and_then(|sum| sum.checked_mul((operations.len() + 1) as u64))
            .is_none_or(|total| total > MAX_PROJECTION_BYTES)
        {
            return Err(
                "transaction journal exceeds the 16 MiB aggregate projected preimage bound"
                    .to_owned(),
            );
        }
        let envelope: Envelope<Journal> = serde_json::from_value(value)
            .map_err(|error| format!("transaction journal is corrupt: {error}"))?;
        let encoded = serde_json::to_vec(&envelope.payload)
            .map_err(|error| format!("verify transaction journal: {error}"))?;
        if envelope.sha256 != digest_hex(Sha256::digest(&encoded)) {
            return Err("transaction journal integrity digest does not match".to_owned());
        }
        if envelope.payload.version != STORE_VERSION {
            return Err("transaction journal version is unsupported".to_owned());
        }
        if envelope.payload.repository_identity.root != path_string(&self.root)
            || envelope.payload.native_root != NativeRootIdentity::capture(&self.root)?
            || envelope.payload.storage_identity != NativeRootIdentity::capture(&self.anchor)?
        {
            return Err(
                "transaction journal belongs to a different working-root identity".to_owned(),
            );
        }
        validate_key(&envelope.payload.request_key)?;
        if !valid_digest(&envelope.payload.payload_sha256)
            || !valid_digest(&envelope.payload.plan_token)
            || envelope
                .payload
                .step_sha256
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(
                "transaction journal contains a malformed intent or sequence digest".to_owned(),
            );
        }
        let intent = serde_json::to_vec(&(
            &envelope.payload.repository_identity,
            &envelope.payload.request,
        ))
        .map_err(|error| format!("verify transaction intent: {error}"))?;
        if envelope.payload.payload_sha256 != crate::hex_digest(Sha256::digest(intent)) {
            return Err("transaction journal intent digest does not match its request".to_owned());
        }
        let _ = envelope.payload.before.to_live()?;
        let _ = envelope.payload.projected_after.to_live()?;
        if envelope.payload.step_sha256.len() != envelope.payload.operations.len() + 1
            || envelope.payload.next_operation > envelope.payload.operations.len()
            || (envelope.payload.phase == Phase::Prepared && envelope.payload.next_operation != 0)
            || (envelope.payload.phase == Phase::RollingBack
                && envelope.payload.next_operation >= envelope.payload.operations.len())
        {
            return Err("transaction journal sequence is inconsistent".to_owned());
        }
        Ok(Some(envelope.payload))
    }

    pub(super) fn write_journal(&self, journal: &Journal, create: bool) -> Result<(), String> {
        if journal.version != STORE_VERSION {
            return Err("refusing to write an unsupported transaction journal version".to_owned());
        }
        let payload = serde_json::to_vec(journal)
            .map_err(|error| format!("encode transaction journal: {error}"))?;
        let envelope = Envelope {
            sha256: digest_hex(Sha256::digest(&payload)),
            payload: journal,
        };
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|error| format!("encode transaction journal envelope: {error}"))?;
        if bytes.len() as u64 > MAX_JOURNAL_BYTES {
            return Err("transaction journal exceeds the 256 MiB limit".to_owned());
        }
        atomic_write(&self.journal_path, &bytes, !create)
    }

    pub(super) fn delete_journal(&self) -> Result<(), String> {
        match fs::symlink_metadata(&self.journal_path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("inspect completed transaction journal: {error}")),
            Ok(metadata) if !metadata.is_file() || is_reparse_point(&metadata) => {
                Err("completed transaction journal is not a regular file".to_owned())
            }
            Ok(metadata) => {
                ensure_single_link(&self.journal_path, &metadata)?;
                #[cfg(test)]
                if std::env::var("REPOPACT_TEST_FAIL_JOURNAL_CLEANUP").as_deref() == Ok("1") {
                    return Err("injected terminal journal cleanup failure".to_owned());
                }
                fs::remove_file(&self.journal_path)
                    .map_err(|error| format!("remove completed transaction journal: {error}"))?;
                sync_directory(&self.anchor)
                    .map_err(|error| format!("sync transaction state directory: {error}"))
            }
        }
    }

    pub(super) fn load_receipt(&self, key: &str) -> Result<Option<Receipt>, String> {
        validate_key(key)?;
        let path = self.receipt_path(key);
        let bytes = match read_regular_file(&path, MAX_RECEIPT_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error == "missing" => return Ok(None),
            Err(error) => return Err(error),
        };
        let envelope: Envelope<Receipt> = serde_json::from_slice(&bytes)
            .map_err(|error| format!("terminal receipt is corrupt: {error}"))?;
        let payload = serde_json::to_vec(&envelope.payload)
            .map_err(|error| format!("verify terminal receipt: {error}"))?;
        if envelope.sha256 != digest_hex(Sha256::digest(&payload))
            || envelope.payload.version != STORE_VERSION
            || envelope.payload.request_key != key
        {
            return Err("terminal receipt integrity or identity check failed".to_owned());
        }
        if envelope.payload.repository_identity.root != path_string(&self.root)
            || envelope.payload.native_root != NativeRootIdentity::capture(&self.root)?
            || envelope.payload.storage_identity != NativeRootIdentity::capture(&self.anchor)?
        {
            return Err("terminal receipt belongs to a different working-root identity".to_owned());
        }
        valid_receipt_result(&envelope.payload)?;
        Ok(Some(envelope.payload))
    }

    pub(super) fn receipt_count(&self) -> Result<usize, String> {
        let mut count = 0usize;
        let entries = match fs::read_dir(&self.receipts_path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(format!("enumerate terminal receipts: {error}")),
        };
        for entry in entries {
            let entry = entry.map_err(|error| format!("enumerate terminal receipts: {error}"))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !valid_receipt_name(&name) {
                return Err(format!(
                    "unknown artifact '{}' in terminal receipt store",
                    path_string(&entry.path())
                ));
            }
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| format!("inspect terminal receipt: {error}"))?;
            if !metadata.is_file() || is_reparse_point(&metadata) {
                return Err("terminal receipt is not a regular file".to_owned());
            }
            ensure_single_link(&entry.path(), &metadata)?;
            count = count.saturating_add(1);
            if count > MAX_RECEIPTS {
                return Err(format!(
                    "terminal receipt store exceeds its {MAX_RECEIPTS}-entry bound"
                ));
            }
        }
        Ok(count)
    }

    pub(super) fn write_receipt(&self, receipt: &Receipt) -> Result<(), String> {
        validate_key(&receipt.request_key)?;
        valid_receipt_result(receipt)?;
        if receipt.version != STORE_VERSION
            || receipt.repository_identity.root != path_string(&self.root)
            || receipt.native_root != NativeRootIdentity::capture(&self.root)?
            || receipt.storage_identity != NativeRootIdentity::capture(&self.anchor)?
        {
            return Err(
                "refusing to persist a terminal receipt for a different root identity".to_owned(),
            );
        }
        let count = self.receipt_count()?;
        let path = self.receipt_path(&receipt.request_key);
        if path.exists() {
            let existing = self
                .load_receipt(&receipt.request_key)?
                .ok_or_else(|| "terminal receipt disappeared during verification".to_owned())?;
            if existing == *receipt {
                return Ok(());
            }
            return Err("request key already has a different terminal receipt".to_owned());
        }
        if count >= MAX_RECEIPTS {
            return Err(format!(
                "terminal receipt capacity exhausted at {MAX_RECEIPTS}; no receipt was evicted"
            ));
        }
        let payload = serde_json::to_vec(receipt)
            .map_err(|error| format!("encode terminal receipt: {error}"))?;
        let bytes = serde_json::to_vec(&Envelope {
            payload: receipt,
            sha256: digest_hex(Sha256::digest(&payload)),
        })
        .map_err(|error| format!("encode terminal receipt envelope: {error}"))?;
        if bytes.len() as u64 > MAX_RECEIPT_BYTES {
            return Err("terminal receipt exceeds the 64 KiB limit".to_owned());
        }
        atomic_write(&path, &bytes, false)?;
        Ok(())
    }

    pub(super) fn receipts(&self) -> Result<Vec<Receipt>, String> {
        let mut receipts = Vec::new();
        let entries = match fs::read_dir(&self.receipts_path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(receipts),
            Err(error) => return Err(format!("enumerate terminal receipts: {error}")),
        };
        for entry in entries {
            let entry = entry.map_err(|error| format!("enumerate terminal receipts: {error}"))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !valid_receipt_name(&name) {
                return Err(format!(
                    "unknown artifact '{}' in terminal receipt store",
                    path_string(&entry.path())
                ));
            }
            let bytes = read_regular_file(&entry.path(), MAX_RECEIPT_BYTES)?;
            let envelope: Envelope<Receipt> = serde_json::from_slice(&bytes)
                .map_err(|error| format!("terminal receipt is corrupt: {error}"))?;
            let payload = serde_json::to_vec(&envelope.payload)
                .map_err(|error| format!("verify terminal receipt: {error}"))?;
            if envelope.sha256 != digest_hex(Sha256::digest(&payload))
                || envelope.payload.version != STORE_VERSION
                || self.receipt_path(&envelope.payload.request_key) != entry.path()
                || envelope.payload.repository_identity.root != path_string(&self.root)
                || envelope.payload.native_root != NativeRootIdentity::capture(&self.root)?
                || envelope.payload.storage_identity != NativeRootIdentity::capture(&self.anchor)?
            {
                return Err("terminal receipt integrity or root-identity check failed".to_owned());
            }
            valid_receipt_result(&envelope.payload)?;
            receipts.push(envelope.payload);
        }
        receipts.sort_by(|left, right| left.request_key.cmp(&right.request_key));
        Ok(receipts)
    }

    fn receipt_path(&self, key: &str) -> PathBuf {
        self.receipts_path.join(format!(
            "{}.json",
            digest_hex(Sha256::digest(key.as_bytes()))
        ))
    }
}

fn valid_receipt_result(receipt: &Receipt) -> Result<(), String> {
    if !valid_digest(&receipt.payload_sha256)
        || !valid_digest(&receipt.plan_token)
        || receipt.result.mutation_key.as_deref() != Some(receipt.request_key.as_str())
        || receipt.result.plan_token != receipt.plan_token
    {
        return Err("terminal receipt key, intent, or plan identity is inconsistent".to_owned());
    }
    let terminal = match receipt.result.transaction_outcome.as_str() {
        "committed" => receipt.result.success && !receipt.result.rolled_back,
        "not_applied" => !receipt.result.success && !receipt.result.rolled_back,
        "rejected" => !receipt.result.success && !receipt.result.rolled_back,
        "rolled_back" => !receipt.result.success && receipt.result.rolled_back,
        _ => false,
    };
    if !terminal {
        return Err("terminal receipt contains a nonterminal or inconsistent result".to_owned());
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_receipt_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 69
        && bytes[64..] == *b".json"
        && bytes[..64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn validate_anchor_chain(root: &Path) -> Result<bool, String> {
    for relative in ["work", "work/.repopact-transactions", ANCHOR] {
        let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "inspect transaction path '{}': {error}",
                    path_string(&path)
                ))
            }
            Ok(metadata) if !metadata.is_dir() || is_reparse_point(&metadata) => {
                return Err(format!(
                    "transaction path '{}' is not a regular directory",
                    path_string(&path)
                ))
            }
            Ok(_) => {}
        }
    }
    Ok(true)
}

pub(super) fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty()
        || key.len() > 128
        || key.trim() != key
        || key.bytes().any(|byte| !byte.is_ascii_graphic())
    {
        return Err("mutation request key must contain 1 to 128 printable ASCII bytes".to_owned());
    }
    Ok(())
}

fn ensure_anchor(root: &Path) -> Result<PathBuf, String> {
    ensure_repository_directory(root, Path::new("work"))?;
    ensure_repository_directory(root, Path::new("work/.repopact-transactions"))?;
    #[cfg(windows)]
    {
        let proposal_parent = root.join("work/.repopact-transactions");
        check_private_directory(&proposal_parent).map_err(|error| {
            format!("existing proposal transaction anchor has an unsafe ACL; its contents were preserved and no ACL repair was attempted: {error}")
        })?;
    }
    ensure_repository_directory(root, Path::new(ANCHOR))?;
    let anchor = root.join(ANCHOR.replace('/', std::path::MAIN_SEPARATOR_STR));
    #[cfg(windows)]
    check_private_directory(&anchor).map_err(|error| {
        format!("existing ordinary transaction directory has an unsafe ACL; its contents were preserved and no ACL repair was attempted: {error}")
    })?;
    #[cfg(not(windows))]
    check_private_directory(&anchor)?;
    Ok(anchor)
}

pub(super) fn ensure_proposal_anchor(root: &Path) -> Result<PathBuf, String> {
    let relative = Path::new("work/.repopact-transactions");
    ensure_repository_directory(root, relative)?;
    let anchor = root.join(relative);
    check_private_directory(&anchor).map_err(|error| {
        format!(
            "existing proposal transaction anchor has unsafe permissions; its contents were preserved and no repair was attempted: {error}"
        )
    })?;
    Ok(anchor)
}

fn ensure_repository_directory(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("transaction directory path is not normalized".to_owned());
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => {}
            Ok(_) => {
                return Err(format!(
                    "transaction directory '{}' is not a regular directory",
                    path_string(&current)
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match create_private_directory(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        let metadata = fs::symlink_metadata(&current).map_err(|error| {
                            format!("inspect concurrently created transaction directory: {error}")
                        })?;
                        if !metadata.is_dir() || is_reparse_point(&metadata) {
                            return Err(format!(
                                "transaction directory '{}' is not a regular directory",
                                path_string(&current)
                            ));
                        }
                    }
                    Err(error) => {
                        return Err(format!(
                            "create transaction directory '{}': {error}",
                            path_string(&current)
                        ));
                    }
                }
            }
            Err(error) => {
                return Err(format!(
                    "inspect transaction directory '{}': {error}",
                    path_string(&current)
                ))
            }
        }
    }
    Ok(())
}

fn ensure_private_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => {
            check_private_directory(path)
        }
        Ok(_) => Err(format!(
            "transaction directory '{}' is not a regular directory",
            path_string(path)
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match create_private_directory(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(format!("create transaction directory: {error}")),
            }
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| format!("inspect transaction directory: {error}"))?;
            if !metadata.is_dir() || is_reparse_point(&metadata) {
                return Err("transaction path is not a regular directory".to_owned());
            }
            check_private_directory(path)
        }
        Err(error) => Err(format!("inspect transaction directory: {error}")),
    }
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(windows)]
fn create_private_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::{
        InitializeSecurityDescriptor, SetSecurityDescriptorControl, SetSecurityDescriptorDacl,
        SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
    };
    use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;

    let user_sid = current_token_sid()
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
    let system_sid = sid_bytes("S-1-5-18")
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
    let admins_sid = sid_bytes("S-1-5-32-544")
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
    let mut acl_bytes = vec![0u8; 512];
    let acl = acl_bytes
        .as_mut_ptr()
        .cast::<windows_sys::Win32::Security::ACL>();
    if unsafe {
        windows_sys::Win32::Security::InitializeAcl(
            acl,
            acl_bytes.len() as u32,
            windows_sys::Win32::Security::ACL_REVISION,
        )
    } == 0
    {
        return Err(io::Error::other(
            "unable to initialize a private transaction DACL",
        ));
    }
    for sid in [&user_sid, &system_sid, &admins_sid] {
        if unsafe {
            windows_sys::Win32::Security::AddAccessAllowedAceEx(
                acl,
                windows_sys::Win32::Security::ACL_REVISION,
                windows_sys::Win32::Security::OBJECT_INHERIT_ACE
                    | windows_sys::Win32::Security::CONTAINER_INHERIT_ACE,
                windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS,
                sid.as_ptr() as *mut _,
            )
        } == 0
        {
            return Err(io::Error::other(
                "unable to add a trusted principal to the transaction DACL",
            ));
        }
    }
    let mut descriptor = SECURITY_DESCRIPTOR::default();
    let descriptor_ptr = (&mut descriptor as *mut SECURITY_DESCRIPTOR).cast();
    if unsafe { InitializeSecurityDescriptor(descriptor_ptr, 1) } == 0
        || unsafe { SetSecurityDescriptorDacl(descriptor_ptr, 1, acl, 0) } == 0
        || unsafe {
            SetSecurityDescriptorControl(descriptor_ptr, SE_DACL_PROTECTED, SE_DACL_PROTECTED)
        } == 0
    {
        return Err(io::Error::other(
            "unable to initialize a protected transaction security descriptor",
        ));
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: (&mut descriptor as *mut SECURITY_DESCRIPTOR).cast(),
        bInheritHandle: 0,
    };
    let mut wide_path = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide_path.push(0);
    if unsafe { CreateDirectoryW(wide_path.as_ptr(), &attributes) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(unix, windows)))]
fn create_private_directory(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "private transaction-directory permissions are unsupported on this platform",
    ))
}

#[cfg(unix)]
pub(super) fn check_private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let mode = metadata.mode();
    let current_uid = unsafe { libc::geteuid() };
    if metadata.uid() != current_uid || mode & 0o077 != 0 {
        return Err(format!(
            "transaction directory '{}' is not owned by this user with mode 0700",
            path_string(path)
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn check_private_directory(path: &Path) -> Result<(), String> {
    check_private_acl(path)
}

#[cfg(windows)]
fn current_token_sid() -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetLengthSid, GetTokenInformation, IsValidSid, TokenUser, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token: HANDLE = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(format!(
            "unable to open the current Windows token: {}",
            io::Error::last_os_error()
        ));
    }
    let mut needed = 0u32;
    unsafe {
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
    }
    if needed < std::mem::size_of::<TOKEN_USER>() as u32 {
        unsafe { CloseHandle(token) };
        return Err("current Windows token user SID is unavailable".to_owned());
    }
    let mut buffer = vec![0u8; needed as usize];
    let read = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } != 0;
    unsafe { CloseHandle(token) };
    if !read {
        return Err(format!(
            "unable to read the current Windows token user SID: {}",
            io::Error::last_os_error()
        ));
    }
    let user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
    if user.User.Sid.is_null() || unsafe { IsValidSid(user.User.Sid) } == 0 {
        return Err("current Windows token contains a malformed user SID".to_owned());
    }
    let length = unsafe { GetLengthSid(user.User.Sid) } as usize;
    if length == 0 || length > buffer.len() {
        return Err("current Windows token user SID has an invalid length".to_owned());
    }
    let bytes = unsafe { std::slice::from_raw_parts(user.User.Sid.cast::<u8>(), length) };
    Ok(bytes.to_vec())
}

#[cfg(windows)]
fn sid_bytes(value: &str) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows_sys::Win32::Security::GetLengthSid;

    let mut wide = value.encode_utf16().collect::<Vec<_>>();
    wide.push(0);
    let mut sid = std::ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(wide.as_ptr(), &mut sid) } == 0 || sid.is_null() {
        return Err(format!("unable to construct trusted Windows SID {value}"));
    }
    let length = unsafe { GetLengthSid(sid) } as usize;
    let bytes = if length == 0 {
        Err(format!("trusted Windows SID {value} has an invalid length"))
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(sid.cast::<u8>(), length) }.to_vec();
        Ok(bytes)
    };
    unsafe { LocalFree(sid as HLOCAL) };
    bytes
}

#[cfg(windows)]
fn check_private_acl(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HLOCAL};
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        AclSizeInformation, EqualSid, GetAce, GetAclInformation, GetLengthSid,
        GetSecurityDescriptorDacl, IsValidAcl, IsValidSecurityDescriptor, IsValidSid,
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION,
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;

    let mut wide_path = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide_path.push(0);
    let mut owner_sid: PSID = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner_sid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS || descriptor.is_null() {
        if !descriptor.is_null() {
            unsafe { LocalFree(descriptor as HLOCAL) };
        }
        return Err(format!(
            "unable to inspect native transaction security descriptor (Windows error {status})"
        ));
    }
    let result = (|| {
        if unsafe { IsValidSecurityDescriptor(descriptor) } == 0
            || owner_sid.is_null()
            || unsafe { IsValidSid(owner_sid) } == 0
        {
            return Err("transaction security descriptor or owner SID is malformed".to_owned());
        }
        let user_sid = current_token_sid()?;
        let user = user_sid.as_ptr() as PSID;
        if unsafe { EqualSid(owner_sid, user) } == 0 {
            return Err("transaction path is not owned by the current token user".to_owned());
        }
        let system_sid = sid_bytes("S-1-5-18")?;
        let admins_sid = sid_bytes("S-1-5-32-544")?;
        let owner_rights_sid = sid_bytes("S-1-3-4")?;
        let mut present = 0;
        let mut defaulted = 0;
        let mut dacl = std::ptr::null_mut();
        if unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) }
            == 0
            || present == 0
            || dacl.is_null()
            || unsafe { IsValidAcl(dacl) } == 0
        {
            return Err("transaction DACL is absent, NULL, or malformed".to_owned());
        }
        let mut information = ACL_SIZE_INFORMATION::default();
        if unsafe {
            GetAclInformation(
                dacl,
                (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
        {
            return Err("unable to inspect native transaction DACL entries".to_owned());
        }
        let mut current_user_granted = false;
        for index in 0..information.AceCount {
            let mut raw_ace = std::ptr::null_mut();
            if unsafe { GetAce(dacl, index, &mut raw_ace) } == 0 || raw_ace.is_null() {
                return Err("transaction DACL contains a malformed ACE".to_owned());
            }
            let header = unsafe { &*(raw_ace.cast::<ACE_HEADER>()) };
            if header.AceType != 0 && header.AceType != 1 {
                return Err("transaction DACL contains an unsupported ACE type".to_owned());
            }
            if header.AceType == 1 {
                return Err(
                    "transaction DACL contains a deny ACE; permissions could not be proven"
                        .to_owned(),
                );
            }
            if (header.AceSize as usize) < std::mem::size_of::<ACCESS_ALLOWED_ACE>() {
                return Err("transaction DACL contains a truncated access ACE".to_owned());
            }
            let ace = unsafe { &*(raw_ace.cast::<ACCESS_ALLOWED_ACE>()) };
            let ace_sid = std::ptr::addr_of!(ace.SidStart) as PSID;
            let sid_offset = (ace_sid as usize).saturating_sub(raw_ace as usize);
            if unsafe { IsValidSid(ace_sid) } == 0 {
                return Err("transaction DACL contains a malformed principal SID".to_owned());
            }
            let sid_length = unsafe { GetLengthSid(ace_sid) } as usize;
            if sid_length == 0 || sid_offset.saturating_add(sid_length) > header.AceSize as usize {
                return Err("transaction DACL contains a malformed principal SID".to_owned());
            }
            let is_user = unsafe { EqualSid(ace_sid, user) } != 0;
            let is_system = unsafe { EqualSid(ace_sid, system_sid.as_ptr() as PSID) } != 0;
            let is_admins = unsafe { EqualSid(ace_sid, admins_sid.as_ptr() as PSID) } != 0;
            let is_owner_rights =
                unsafe { EqualSid(ace_sid, owner_rights_sid.as_ptr() as PSID) } != 0;
            if !(is_user || is_system || is_admins || is_owner_rights) {
                return Err("transaction DACL grants or names an untrusted principal".to_owned());
            }
            if (is_user || is_owner_rights) && ace.Mask & FILE_ALL_ACCESS != FILE_ALL_ACCESS {
                return Err(
                    "transaction DACL does not grant the current owner full transaction access"
                        .to_owned(),
                );
            }
            if header.AceType == 0
                && header.AceFlags & (windows_sys::Win32::Security::INHERIT_ONLY_ACE as u8) == 0
                && (is_user || is_owner_rights)
            {
                current_user_granted = true;
            }
        }
        if !current_user_granted {
            return Err("transaction DACL does not grant the current token user access".to_owned());
        }
        Ok(())
    })();
    unsafe { LocalFree(descriptor as HLOCAL) };
    result.map_err(|error: String| {
        format!(
            "transaction ACL check failed for '{}': {error}",
            path_string(path)
        )
    })
}

#[cfg(not(any(unix, windows)))]
pub(super) fn check_private_directory(_path: &Path) -> Result<(), String> {
    Err("private transaction-directory permissions are unsupported on this platform".to_owned())
}

fn read_regular_file(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err("missing".to_owned()),
        Err(error) => return Err(format!("inspect transaction file: {error}")),
    };
    if !metadata.is_file() || is_reparse_point(&metadata) {
        return Err("transaction path is not a regular file".to_owned());
    }
    ensure_single_link(path, &metadata)?;
    check_private_file(path, &metadata)?;
    if metadata.len() > maximum {
        return Err("transaction file exceeds its size limit".to_owned());
    }
    let bytes = fs::read(path).map_err(|error| format!("read transaction file: {error}"))?;
    if bytes.len() as u64 > maximum || bytes.len() as u64 != metadata.len() {
        return Err("transaction file changed size while it was read".to_owned());
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8], replace: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "transaction file has no parent".to_owned())?;
    let temporary = parent.join(format!(
        ".journal-{}-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    let write_result = (|| -> Result<(), String> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("create transaction staging file: {error}"))?;
        file.write_all(bytes)
            .map_err(|error| format!("write transaction staging file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync transaction staging file: {error}"))?;
        #[cfg(test)]
        if is_receipt_path(path)
            && std::env::var("REPOPACT_TEST_RECEIPT_FAILURE").as_deref() == Ok("before_publish")
        {
            return Err("injected receipt failure before publication".to_owned());
        }
        if replace {
            atomic_replace(&temporary, path)
                .map_err(|error| format!("publish transaction file: {error}"))?;
        } else {
            match fs::symlink_metadata(path) {
                Ok(_) => return Err("transaction journal already exists".to_owned()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!("inspect transaction journal destination: {error}"))
                }
            }
            fs::rename(&temporary, path)
                .map_err(|error| format!("publish transaction file: {error}"))?;
        }
        #[cfg(test)]
        if is_receipt_path(path)
            && std::env::var("REPOPACT_TEST_RECEIPT_FAILURE").as_deref() == Ok("after_publish")
        {
            return Err("injected receipt error after publication".to_owned());
        }
        sync_directory(parent)
            .map_err(|error| format!("sync transaction state directory: {error}"))?;
        Ok(())
    })();
    if write_result.is_err() {
        // A failed or interrupted staging artifact is intentionally retained
        // for explicit inspection rather than removed by a broad cleanup.
    }
    write_result
}

#[cfg(test)]
fn is_receipt_path(path: &Path) -> bool {
    path.parent().and_then(Path::file_name) == Some(std::ffi::OsStr::new(RECEIPTS_DIR))
}

#[cfg(windows)]
fn atomic_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, target: *const u16, flags: u32) -> i32;
    }
    let mut source = from.as_os_str().encode_wide().collect::<Vec<_>>();
    let mut target = to.as_os_str().encode_wide().collect::<Vec<_>>();
    source.push(0);
    target.push(0);
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn parse_relative(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "transaction path '{value}' is not normalized and relative"
        ));
    }
    if lossless_path_string(&path)? != value {
        return Err(format!(
            "transaction path '{value}' does not round-trip through the native path format"
        ));
    }
    Ok(path)
}

pub(super) fn lossless_path_string(path: &Path) -> Result<String, String> {
    let native = path.to_str().ok_or_else(|| {
        "transaction path is not valid UTF-8 and cannot be journaled losslessly".to_owned()
    })?;
    let normalized = path_string(path);
    let round_trip = PathBuf::from(normalized.replace('/', std::path::MAIN_SEPARATOR_STR));
    if round_trip != path {
        return Err(format!(
            "transaction path '{}' cannot be represented losslessly in the journal",
            native
        ));
    }
    Ok(normalized)
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return metadata.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    false
}

fn ensure_single_link(path: &Path, _metadata: &fs::Metadata) -> Result<(), String> {
    #[cfg(unix)]
    let links = {
        use std::os::unix::fs::MetadataExt;
        _metadata.nlink()
    };
    #[cfg(windows)]
    let links = {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let file =
            File::open(path).map_err(|error| format!("inspect transaction-file links: {error}"))?;
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
            return Err("cannot verify transaction-file link count".to_owned());
        }
        info.nNumberOfLinks as u64
    };
    #[cfg(not(any(unix, windows)))]
    let links = {
        let _ = (path, metadata);
        return Err("transaction-file link count is unsupported on this platform".to_owned());
    };
    if links != 1 {
        return Err(format!(
            "transaction file '{}' has {links} hard links",
            path_string(path)
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn windows_directory_identity(path: &Path) -> Result<(u64, u64), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const std::ffi::c_void,
            disposition: u32,
            flags: u32,
            template: *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
        fn GetFileInformationByHandle(
            handle: *mut std::ffi::c_void,
            information: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    let mut wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide.push(0);
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == -1isize as *mut std::ffi::c_void {
        return Err("cannot open working root to capture its native identity".to_owned());
    }
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    let succeeded = unsafe { GetFileInformationByHandle(handle, &mut info) } != 0;
    unsafe {
        CloseHandle(handle);
    }
    if !succeeded || info.dwFileAttributes & 0x400 != 0 {
        return Err("cannot verify working-root native identity".to_owned());
    }
    Ok((
        u64::from(info.dwVolumeSerialNumber),
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}

#[cfg(unix)]
pub(super) fn check_private_file(_path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let mode = metadata.mode();
    if metadata.uid() != unsafe { libc::geteuid() } || mode & 0o077 != 0 {
        return Err("transaction file is not owned by this user with mode 0600".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn check_private_file(path: &Path, _metadata: &fs::Metadata) -> Result<(), String> {
    check_private_directory(path)
}

#[cfg(not(any(unix, windows)))]
fn check_private_file(_path: &Path, _metadata: &fs::Metadata) -> Result<(), String> {
    Err("private transaction-file permissions are unsupported on this platform".to_owned())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 || !value.is_ascii() {
        return Err("transaction preimage encoding is malformed".to_owned());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let hi = hex_value(pair[0])
            .ok_or_else(|| "transaction preimage encoding is malformed".to_owned())?;
        let lo = hex_value(pair[1])
            .ok_or_else(|| "transaction preimage encoding is malformed".to_owned())?;
        bytes.push((hi << 4) | lo);
    }
    Ok(bytes)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub(super) fn digest_hex(value: impl AsRef<[u8]>) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
