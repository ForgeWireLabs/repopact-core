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
use sha2::{Digest, Sha256};

use crate::{MutationRequest, MutationResult, PathOperation, PreimageEntry, Preimages};

pub(super) const STORE_VERSION: u32 = 1;
pub(super) const MAX_RECEIPTS: usize = 1024;
pub(super) const MAX_PREIMAGE_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_JOURNAL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 64 * 1024;
const MAX_PREIMAGE_ENTRIES: usize = 100_000;
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
            canonical_path: path_string(&canonical),
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
            .map(|path| path_string(path))
            .collect();
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
                path: path_string(path),
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
        let envelope: Envelope<Journal> = serde_json::from_slice(&bytes)
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
    for relative in [
        Path::new("work"),
        Path::new("work/.repopact-transactions"),
        Path::new(ANCHOR),
    ] {
        ensure_repository_directory(root, relative)?;
    }
    #[cfg(windows)]
    {
        let proposal_parent = root.join("work/.repopact-transactions");
        if check_private_directory(&proposal_parent).is_err() {
            secure_private_directory(&proposal_parent)?;
        }
    }
    let anchor = root.join(ANCHOR.replace('/', std::path::MAIN_SEPARATOR_STR));
    #[cfg(windows)]
    if check_private_directory(&anchor).is_err() {
        secure_private_directory(&anchor)?;
    }
    check_private_directory(&anchor)?;
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
                match fs::create_dir(&current) {
                    Ok(()) => set_private_directory(&current)?,
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
            match fs::create_dir(path) {
                Ok(()) => set_private_directory(path)?,
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
fn set_private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("secure transaction directory: {error}"))
}

#[cfg(windows)]
fn set_private_directory(path: &Path) -> Result<(), String> {
    secure_private_directory(path)
}

#[cfg(not(any(unix, windows)))]
fn set_private_directory(_path: &Path) -> Result<(), String> {
    Err("private transaction-directory permissions are unsupported on this platform".to_owned())
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
fn secure_private_directory(path: &Path) -> Result<(), String> {
    let sid = current_windows_user_sid()?;
    let system_root = std::env::var_os("SystemRoot")
        .ok_or_else(|| "SystemRoot is unavailable for transaction ACL protection".to_owned())?;
    let system = PathBuf::from(system_root).join("System32");
    let icacls = system.join("icacls.exe");
    let full_current = format!("*{sid}:(OI)(CI)(F)");
    let full_system = "*S-1-5-18:(OI)(CI)(F)";
    let full_admins = "*S-1-5-32-544:(OI)(CI)(F)";
    let output = std::process::Command::new(&icacls)
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(full_current)
        .arg(full_system)
        .arg(full_admins)
        .output()
        .map_err(|error| format!("unable to protect transaction ACL: {error}"))?;
    if !output.status.success() {
        return Err("unable to restrict transaction ACL; persistence is closed".to_owned());
    }
    check_private_directory(path)
}

#[cfg(windows)]
fn current_windows_user_sid() -> Result<String, String> {
    let system_root = std::env::var_os("SystemRoot")
        .ok_or_else(|| "SystemRoot is unavailable for transaction ACL inspection".to_owned())?;
    let system = PathBuf::from(system_root).join("System32");
    let whoami = std::process::Command::new(system.join("whoami.exe"))
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .map_err(|error| format!("unable to identify the current transaction owner: {error}"))?;
    if !whoami.status.success() {
        return Err("unable to identify the current transaction owner".to_owned());
    }
    let identity = String::from_utf8_lossy(&whoami.stdout);
    let sid = identity
        .split(',')
        .nth(1)
        .map(|value| value.trim().trim_matches('"'))
        .filter(|value| value.starts_with("S-1-"))
        .ok_or_else(|| "current transaction owner SID is unavailable".to_owned())?;
    Ok(sid.to_owned())
}

#[cfg(windows)]
fn directory_owner_is_current_user(path: &Path, user_sid: &str) -> Result<bool, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HLOCAL};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSidToSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{EqualSid, PSECURITY_DESCRIPTOR, PSID};

    let mut wide_path = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide_path.push(0);
    let mut owner_sid: PSID = std::ptr::null_mut();
    let mut security_descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            windows_sys::Win32::Security::OWNER_SECURITY_INFORMATION,
            &mut owner_sid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut security_descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        if !security_descriptor.is_null() {
            unsafe {
                LocalFree(security_descriptor as HLOCAL);
            }
        }
        return Err(format!(
            "unable to inspect transaction directory owner (Windows error {status})"
        ));
    }

    let mut wide_sid = user_sid.encode_utf16().collect::<Vec<_>>();
    wide_sid.push(0);
    let mut current_sid: PSID = std::ptr::null_mut();
    let converted = unsafe { ConvertStringSidToSidW(wide_sid.as_ptr(), &mut current_sid) } != 0;
    if !converted {
        unsafe {
            LocalFree(security_descriptor as HLOCAL);
        }
        return Err("unable to parse the current transaction owner SID".to_owned());
    }
    let matches = unsafe { EqualSid(owner_sid, current_sid) } != 0;
    unsafe {
        LocalFree(current_sid as HLOCAL);
        LocalFree(security_descriptor as HLOCAL);
    }
    Ok(matches)
}

#[cfg(windows)]
pub(super) fn check_private_directory(path: &Path) -> Result<(), String> {
    let system_root = std::env::var_os("SystemRoot")
        .ok_or_else(|| "SystemRoot is unavailable for transaction ACL inspection".to_owned())?;
    let icacls = PathBuf::from(system_root)
        .join("System32")
        .join("icacls.exe");
    let output = std::process::Command::new(icacls)
        .arg(path)
        .output()
        .map_err(|error| format!("unable to inspect transaction ACL: {error}"))?;
    if !output.status.success() {
        return Err("unable to inspect transaction ACL; persistence is closed".to_owned());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines().skip(1) {
        let Some((principal, rights)) = line.trim().split_once(':') else {
            continue;
        };
        let principal = principal.trim();
        let rights = rights.trim();
        if rights.contains("(M)")
            || rights.contains("(F)")
            || rights.contains("(W)")
            || rights.contains("(RX)")
            || rights.contains("(R)")
            || rights.contains("(RD)")
        {
            // OWNER RIGHTS is a special Windows SID. It is safe here only when
            // the actual directory owner is the current transaction user.
            if principal.eq_ignore_ascii_case("OWNER RIGHTS") {
                let current_sid = current_windows_user_sid()?;
                if directory_owner_is_current_user(path, &current_sid)? {
                    continue;
                }
            }
            let allowed = principal.ends_with("\\Administrators")
                || principal.ends_with("\\SYSTEM")
                || principal == std::env::var("USERNAME").unwrap_or_default()
                || principal.ends_with(&format!(
                    "\\{}",
                    std::env::var("USERNAME").unwrap_or_default()
                ));
            if !allowed {
                return Err(
                    "transaction ACL grants another principal access; persistence is closed"
                        .to_owned(),
                );
            }
        }
    }
    Ok(())
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
    Ok(path)
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
