use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn git_revision_inputs(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(head) =
        git_path(root, &["rev-parse", "--git-path", "HEAD"]).filter(|path| path.is_file())
    {
        push_unique(&mut paths, head);
    }
    if let Some(packed_refs) =
        git_path(root, &["rev-parse", "--git-path", "packed-refs"]).filter(|path| path.is_file())
    {
        push_unique(&mut paths, packed_refs);
    }

    if let Some(reference) = git_output(root, &["symbolic-ref", "--quiet", "HEAD"])
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        if let Some(reference_path) = git_path(root, &["rev-parse", "--git-path", &reference]) {
            if reference_path.is_file() {
                push_unique(&mut paths, reference_path.clone());
            }
            // Track the nearest existing directory so Cargo notices when a
            // packed branch becomes loose, or a loose ref is packed/deleted.
            if let Some(parent) = reference_path
                .ancestors()
                .skip(1)
                .find(|path| path.is_dir())
            {
                push_unique(&mut paths, parent.to_path_buf());
            }
        }
    }
    paths
}

pub fn git_revision(root: &Path) -> Option<String> {
    let output = git_output(root, &["rev-parse", "--verify", "HEAD"])?;
    let revision = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    valid_revision(&revision).then(|| revision.to_ascii_lowercase())
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

fn git_path(root: &Path, args: &[&str]) -> Option<PathBuf> {
    let output = git_output(root, args)?;
    let value = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(value.trim());
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

fn git_output(root: &Path, args: &[&str]) -> Option<Output> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    command
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
}

pub fn valid_revision(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
