use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

/// Walk `input_dir` recursively and collect all `.h` files.
/// Also returns one `-I<absdir>` flag per unique directory that contains a
/// header, so clang can resolve cross-file includes.
pub fn discover(input_dir: &Path) -> Result<(Vec<PathBuf>, Vec<String>)> {
    let mut headers = Vec::new();
    let mut include_dirs: HashSet<String> = HashSet::new();

    collect(input_dir, &mut headers, &mut include_dirs)?;

    headers.sort(); // deterministic order

    let mut flags: Vec<String> = include_dirs.into_iter().collect();
    flags.sort();

    Ok((headers, flags))
}

fn collect(dir: &Path, headers: &mut Vec<PathBuf>, dirs: &mut HashSet<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect(&path, headers, dirs)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("h") {
            // Record the parent directory as an include path.
            if let Some(parent) = path.parent() {
                if let Ok(abs) = parent.canonicalize() {
                    dirs.insert(abs.to_string_lossy().into_owned());
                }
            }
            headers.push(path);
        }
    }
    Ok(())
}
