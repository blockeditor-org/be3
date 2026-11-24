//! Writes OUT, the SHA-256 of every file under DIR as JSON, an object from each file's path
//! relative to DIR to its hash in lower-case hex: a program's `app-hashes.json`
//! (buck/program/defs.bzl's bundle_hashes), which a release publishes beside its apps so
//! `be` can check each app file it downloads (guides/apps/runtime.md, "Headless").

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use be_lib_sha256::sha256_read;

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [out, dir] = arguments else {
        return Err("usage: hashes OUT DIR".into());
    };
    let mut files = Vec::new();
    walk(Path::new(dir), Path::new(""), &mut files)?;
    let mut hashes = BTreeMap::new();
    for relative in files {
        let name = relative
            .to_str()
            .ok_or_else(|| format!("{} is not UTF-8", relative.display()))?
            .replace('\\', "/");
        let path = Path::new(dir).join(&relative);
        let (hash, _) = std::fs::File::open(&path)
            .and_then(sha256_read)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        hashes.insert(name, hash);
    }
    let json = serde_json::to_string_pretty(&hashes).map_err(|error| error.to_string())?;
    std::fs::write(out, json).map_err(|error| format!("cannot write {out}: {error}"))?;
    Ok(ExitCode::SUCCESS)
}

fn walk(root: &Path, relative: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let dir = root.join(relative);
    let entries = std::fs::read_dir(&dir)
        .map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let inner = relative.join(entry.file_name());
        // A program's files are symlinks into the build's outputs: follow them.
        let metadata = std::fs::metadata(entry.path()).map_err(|error| error.to_string())?;
        match metadata.is_dir() {
            true => walk(root, &inner, files)?,
            false => files.push(inner),
        }
    }
    Ok(())
}
