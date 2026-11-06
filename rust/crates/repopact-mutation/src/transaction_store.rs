//! Durable per-repository mutation receipts and crash-recovery preimages.
//!
//! The transaction lock serializes access to this store. A request record is
//! created before snapshotting, and it is atomically changed to `prepared`
//! only after every preimage has been written. Repository writes never start
//! before that transition.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{MutationResult, Preimages};

const STORE_VERSION: u32 = 1;
const RECORD_FILE: &str = "receipt.json";
static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Planning,
    Prepared,
    RollingBack,
    Committed,
    RolledBack,
    Rejected,
}

impl Phase {
    pub(super) fn is_terminal(self) -> bool {
        matches!(self, Self::Committed | Self::RolledBack | Self::Rejected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PreimageFile {
    pub path: String,
    pub blob: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct TransactionRecord {
    pub version: u32,
    pub request_key: String,
    pub payload_sha256: String,
    pub phase: Phase,
    #[serde(default)]
    pub plan_token: String,
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub preimage_directories: Vec<String>,
    #[serde(default)]
    pub preimage_files: Vec<PreimageFile>,
    #[serde(default)]
    pub result: Option<MutationResult>,
}

pub(super) struct TransactionStore {
    requests: PathBuf,
}

impl TransactionStore {
    pub(super) fn open(repository_root: &Path) -> Result<Self, String> {
        let repository_root = fs::canonicalize(repository_root)
            .map_err(|error| format!("canonicalize transaction repository root: {error}"))?;
        let root_key = repository_root
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        let repository_key = digest_hex(Sha256::digest(root_key.as_bytes()));

        #[cfg(test)]
        let base = repository_root.join(".repopact-transaction-test-state");
        #[cfg(not(test))]
        let base = user_state_base()?.join("RepoPact").join("transactions-v1");

        let base = if base.exists() {
            fs::canonicalize(&base)
                .map_err(|error| format!("canonicalize transaction state root: {error}"))?
        } else {
            create_directory_chain(&base)?;
            fs::canonicalize(&base)
                .map_err(|error| format!("canonicalize transaction state root: {error}"))?
        };
        let repository_state = base.join(repository_key);
        ensure_directory(&repository_state)?;
        let requests = repository_state.join("requests");
        ensure_directory(&requests)?;
        Ok(Self { requests })
    }

    fn request_directory(&self, request_key: &str) -> PathBuf {
        self.requests
            .join(digest_hex(Sha256::digest(request_key.as_bytes())))
    }

    pub(super) fn load(&self, request_key: &str) -> Result<Option<TransactionRecord>, String> {
        let directory = self.request_directory(request_key);
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("inspect request receipt directory: {error}")),
            Ok(metadata) if !metadata.is_dir() || is_reparse_point(&metadata) => {
                return Err("request receipt path is not a regular directory".to_owned())
            }
            Ok(_) => {}
        }
        let record_path = directory.join(RECORD_FILE);
        let metadata = fs::symlink_metadata(&record_path)
            .map_err(|error| format!("inspect request receipt: {error}"))?;
        if !metadata.is_file() || is_reparse_point(&metadata) {
            return Err("request receipt is not a regular file".to_owned());
        }
        let bytes =
            fs::read(&record_path).map_err(|error| format!("read request receipt: {error}"))?;
        let record: TransactionRecord = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode request receipt: {error}"))?;
        self.validate_record(request_key, &directory, &record)?;
        Ok(Some(record))
    }

    pub(super) fn records(&self) -> Result<Vec<TransactionRecord>, String> {
        let entries = fs::read_dir(&self.requests)
            .map_err(|error| format!("enumerate request receipts: {error}"))?;
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| format!("enumerate request receipts: {error}"))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".pending-") {
                let metadata = fs::symlink_metadata(entry.path())
                    .map_err(|error| format!("inspect temporary receipt directory: {error}"))?;
                if !metadata.is_dir() || is_reparse_point(&metadata) {
                    return Err("temporary receipt path is not a regular directory".to_owned());
                }
                fs::remove_dir_all(entry.path())
                    .map_err(|error| format!("remove incomplete receipt directory: {error}"))?;
                continue;
            }
            paths.push(entry.path());
        }
        paths.sort();
        let mut records = Vec::with_capacity(paths.len());
        for path in paths {
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("inspect request receipt directory: {error}"))?;
            if !metadata.is_dir() || is_reparse_point(&metadata) {
                return Err(format!(
                    "request receipt entry '{}' is not a regular directory",
                    path.display()
                ));
            }
            let record_path = path.join(RECORD_FILE);
            let record_metadata = fs::symlink_metadata(&record_path)
                .map_err(|error| format!("inspect request receipt: {error}"))?;
            if !record_metadata.is_file() || is_reparse_point(&record_metadata) {
                return Err(format!(
                    "request receipt '{}' is not a regular file",
                    record_path.display()
                ));
            }
            let bytes =
                fs::read(&record_path).map_err(|error| format!("read request receipt: {error}"))?;
            let record: TransactionRecord = serde_json::from_slice(&bytes)
                .map_err(|error| format!("decode request receipt: {error}"))?;
            self.validate_record(&record.request_key, &path, &record)?;
            records.push(record);
        }
        Ok(records)
    }

    pub(super) fn create(
        &self,
        request_key: &str,
        payload_sha256: String,
    ) -> Result<TransactionRecord, String> {
        let destination = self.request_directory(request_key);
        if destination.exists() {
            return Err("request receipt already exists".to_owned());
        }
        let temporary = self.requests.join(format!(
            ".pending-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&temporary)
            .map_err(|error| format!("create temporary request receipt: {error}"))?;
        set_private_directory(&temporary)?;
        let record = TransactionRecord {
            version: STORE_VERSION,
            request_key: request_key.to_owned(),
            payload_sha256,
            phase: Phase::Planning,
            plan_token: String::new(),
            roots: Vec::new(),
            preimage_directories: Vec::new(),
            preimage_files: Vec::new(),
            result: None,
        };
        let encoded = serde_json::to_vec(&record)
            .map_err(|error| format!("encode request receipt: {error}"))?;
        if let Err(error) = write_new_file(&temporary.join(RECORD_FILE), &encoded)
            .and_then(|()| sync_directory(&temporary))
            .and_then(|()| fs::rename(&temporary, &destination))
            .and_then(|()| sync_directory(&self.requests))
        {
            let _ = fs::remove_dir_all(&temporary);
            return Err(format!("persist initial request receipt: {error}"));
        }
        Ok(record)
    }

    pub(super) fn save(&self, record: &TransactionRecord) -> Result<(), String> {
        self.validate_record(
            &record.request_key,
            &self.request_directory(&record.request_key),
            record,
        )?;
        let path = self
            .request_directory(&record.request_key)
            .join(RECORD_FILE);
        let encoded = serde_json::to_vec(record)
            .map_err(|error| format!("encode request receipt: {error}"))?;
        atomic_replace(&path, &encoded)
    }

    pub(super) fn write_preimages(
        &self,
        record: &TransactionRecord,
        preimages: &Preimages,
    ) -> Result<(Vec<String>, Vec<String>, Vec<PreimageFile>), String> {
        let directory = self
            .request_directory(&record.request_key)
            .join("preimages");
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if !metadata.is_dir() || is_reparse_point(&metadata) => {
                return Err("preimage store is not a regular directory".to_owned())
            }
            Ok(_) => fs::remove_dir_all(&directory)
                .map_err(|error| format!("replace incomplete preimage store: {error}"))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect preimage store: {error}")),
        }
        fs::create_dir(&directory).map_err(|error| format!("create preimage store: {error}"))?;
        set_private_directory(&directory)?;
        let mut files = Vec::with_capacity(preimages.files.len());
        for (index, (path, bytes)) in preimages.files.iter().enumerate() {
            let blob = format!("{index:08}.bin");
            write_new_file(&directory.join(&blob), bytes)
                .map_err(|error| format!("persist preimage '{}': {error}", path.display()))?;
            files.push(PreimageFile {
                path: path_string(path),
                blob,
                sha256: digest_hex(Sha256::digest(bytes)),
            });
        }
        sync_directory(&directory).map_err(|error| format!("sync preimage store: {error}"))?;
        Ok((
            preimages
                .directories
                .iter()
                .map(|path| path_string(path))
                .collect(),
            preimages
                .files
                .keys()
                .map(|path| path_string(path))
                .collect(),
            files,
        ))
    }

    pub(super) fn load_preimages(
        &self,
        record: &TransactionRecord,
        max_bytes: u64,
    ) -> Result<Preimages, String> {
        let directory = self
            .request_directory(&record.request_key)
            .join("preimages");
        let metadata = fs::symlink_metadata(&directory)
            .map_err(|error| format!("inspect transaction preimages: {error}"))?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err("transaction preimages are not a regular directory".to_owned());
        }
        let directories = record
            .preimage_directories
            .iter()
            .map(|path| parse_relative_path(path))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let mut files = BTreeMap::new();
        let mut total = 0u64;
        for file in &record.preimage_files {
            let relative = parse_relative_path(&file.path)?;
            if file.blob.contains('/') || file.blob.contains('\\') || !file.blob.ends_with(".bin") {
                return Err("transaction preimage blob name is invalid".to_owned());
            }
            let blob_path = directory.join(&file.blob);
            let metadata = fs::symlink_metadata(&blob_path)
                .map_err(|error| format!("inspect transaction preimage blob: {error}"))?;
            if !metadata.is_file() || is_reparse_point(&metadata) {
                return Err("transaction preimage blob is not a regular file".to_owned());
            }
            total = total.saturating_add(metadata.len());
            if total > max_bytes {
                return Err("transaction preimages exceed the recovery byte limit".to_owned());
            }
            let bytes = fs::read(&blob_path)
                .map_err(|error| format!("read transaction preimage blob: {error}"))?;
            if digest_hex(Sha256::digest(&bytes)) != file.sha256 {
                return Err(format!(
                    "transaction preimage '{}' failed its digest check",
                    file.path
                ));
            }
            if files.insert(relative, bytes).is_some() {
                return Err("transaction receipt contains duplicate preimage paths".to_owned());
            }
        }
        Ok(Preimages { files, directories })
    }

    pub(super) fn remove_preimages(&self, record: &TransactionRecord) -> Result<(), String> {
        let directory = self
            .request_directory(&record.request_key)
            .join("preimages");
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("inspect transaction preimages: {error}")),
            Ok(metadata) if !metadata.is_dir() || is_reparse_point(&metadata) => {
                Err("transaction preimages are not a regular directory".to_owned())
            }
            Ok(_) => fs::remove_dir_all(&directory)
                .map_err(|error| format!("remove completed transaction preimages: {error}")),
        }
    }

    fn validate_record(
        &self,
        request_key: &str,
        directory: &Path,
        record: &TransactionRecord,
    ) -> Result<(), String> {
        if record.version != STORE_VERSION {
            return Err(format!(
                "unsupported transaction receipt version {}",
                record.version
            ));
        }
        if record.request_key != request_key {
            return Err("request receipt key does not match its directory".to_owned());
        }
        if self.request_directory(request_key) != directory {
            return Err("request receipt directory does not match its key".to_owned());
        }
        Ok(())
    }
}

#[cfg(all(windows, not(test)))]
fn user_state_base() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "LOCALAPPDATA is unavailable for durable transaction receipts".to_owned())
}

#[cfg(all(not(test), not(windows)))]
fn user_state_base() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(path));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".local").join("state"));
    }
    Err("no user state directory is available for durable transaction receipts".to_owned())
}

fn create_directory_chain(path: &Path) -> Result<(), String> {
    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        missing.push(cursor.to_path_buf());
        cursor = cursor
            .parent()
            .ok_or_else(|| "transaction state path has no existing ancestor".to_owned())?;
    }
    let metadata = fs::symlink_metadata(cursor)
        .map_err(|error| format!("inspect transaction state ancestor: {error}"))?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err("transaction state ancestor is not a regular directory".to_owned());
    }
    for directory in missing.into_iter().rev() {
        fs::create_dir(&directory)
            .map_err(|error| format!("create transaction state directory: {error}"))?;
        set_private_directory(&directory)?;
    }
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => Ok(()),
        Ok(_) => Err(format!(
            "transaction state path '{}' is not a regular directory",
            path.display()
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(path)
                .map_err(|error| format!("create transaction state directory: {error}"))?;
            set_private_directory(path)?;
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| format!("inspect transaction state directory: {error}"))?;
            if metadata.is_dir() && !is_reparse_point(&metadata) {
                Ok(())
            } else {
                Err("created transaction state path changed filesystem kind".to_owned())
            }
        }
        Err(error) => Err(format!("inspect transaction state directory: {error}")),
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("protect transaction state directory: {error}"))
}

#[cfg(not(unix))]
fn set_private_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "transaction receipt path has no parent".to_owned())?;
    let temporary = parent.join(format!(
        ".receipt-{}-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    write_new_file(&temporary, bytes)
        .map_err(|error| format!("write temporary transaction receipt: {error}"))?;
    if let Err(error) = atomic_rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("replace transaction receipt: {error}"));
    }
    sync_directory(parent).map_err(|error| format!("sync transaction receipt directory: {error}"))
}

#[cfg(windows)]
fn atomic_rename(from: &Path, to: &Path) -> io::Result<()> {
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
    let succeeded = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_rename(from: &Path, to: &Path) -> io::Result<()> {
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

fn parse_relative_path(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "transaction path '{}' is not normalized and relative",
            value
        ));
    }
    Ok(path)
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
