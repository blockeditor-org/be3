use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::helper::{self, Manifest};
use crate::net::Http;

const PART: &str = "part";
const FILES: &str = "files";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Slot {
    PullRequest(u64),
    Main,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Installed {
    pub(crate) slot: Slot,
    pub(crate) commit: String,
    pub(crate) product: String,
}

pub(crate) fn installed(record: &Path) -> Option<Installed> {
    let document = fs::read(record).ok()?;
    serde_json::from_slice(&document).ok()
}

pub(crate) fn remember(record: &Path, installed: Option<&Installed>) -> Result<(), String> {
    match installed {
        Some(installed) => {
            let document = serde_json::to_vec(installed).map_err(|error| error.to_string())?;
            if let Some(parent) = record.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(record, document).map_err(|error| error.to_string())
        }
        None => remove(record),
    }
}

pub(crate) fn cached(cache: &Path, sha256: &str) -> PathBuf {
    cache.join(FILES).join(sha256)
}

pub(crate) fn missing(cache: &Path, manifest: &Manifest) -> u64 {
    manifest
        .files
        .iter()
        .filter(|file| !cached(cache, &file.sha256).exists())
        .map(|file| file.size)
        .sum()
}

pub(crate) fn fetch(
    cache: &Path,
    manifest: &Manifest,
    http: &mut dyn Http,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    let total = missing(cache, manifest);
    let mut done = 0;
    for file in &manifest.files {
        let target = cached(cache, &file.sha256);
        if target.exists() {
            continue;
        }
        let before = done;
        let mut report = |written: u64| progress(before + written, total);
        download(
            http,
            &helper::file_url(&file.sha256),
            &file.sha256,
            &target,
            &mut report,
        )?;
        done += file.size;
    }
    progress(total, total);
    Ok(())
}

fn download(
    http: &mut dyn Http,
    url: &str,
    hash: &str,
    target: &Path,
    progress: &mut dyn FnMut(u64),
) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut part = target.as_os_str().to_owned();
    part.push(".");
    part.push(PART);
    let part = PathBuf::from(part);
    remove(&part)?;
    let file = File::create(&part).map_err(|error| error.to_string())?;
    let mut hashing = Hashing {
        file,
        hasher: Sha256::new(),
        written: 0,
        progress,
    };
    let fetched = http.send("GET", url, &[], &mut hashing);
    let Hashing { file, hasher, .. } = hashing;
    let failure = match fetched {
        Err(error) => Some(error),
        Ok(200) => None,
        Ok(code) => Some(format!("{url} answered {code}")),
    };
    if let Some(error) = failure {
        drop(file);
        let _ = fs::remove_file(&part);
        return Err(error);
    }
    let received = hex(&hasher.finalize());
    if received != hash {
        drop(file);
        let _ = fs::remove_file(&part);
        return Err(format!("{url} held {received}, not {hash}"));
    }
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    remove(target)?;
    fs::rename(&part, target).map_err(|error| error.to_string())
}

struct Hashing<'a> {
    file: File,
    hasher: Sha256,
    written: u64,
    progress: &'a mut dyn FnMut(u64),
}

impl Write for Hashing<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.file.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        self.written += written as u64;
        (self.progress)(self.written);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) fn assemble(cache: &Path, manifest: &Manifest, into: &Path) -> Result<(), String> {
    let mut staging = into.as_os_str().to_owned();
    staging.push(".");
    staging.push(PART);
    let staging = PathBuf::from(staging);
    let _ = fs::remove_dir_all(&staging);
    for file in &manifest.files {
        let target = staging.join(&file.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let source = cached(cache, &file.sha256);
        if fs::hard_link(&source, &target).is_err() {
            fs::copy(&source, &target).map_err(|error| format!("{}: {error}", target.display()))?;
        }
        if file.executable {
            executable(&target)?;
        }
    }
    let _ = fs::remove_dir_all(into);
    fs::rename(&staging, into).map_err(|error| format!("{}: {error}", into.display()))
}

#[cfg(all(unix, not(target_os = "android")))]
fn executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = fs::metadata(path)
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(path, permissions).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn executable(_: &Path) -> Result<(), String> {
    Ok(())
}

pub(crate) fn forget_unused(cache: &Path, kept: &[&Manifest]) {
    let used: std::collections::HashSet<&str> = kept
        .iter()
        .flat_map(|manifest| manifest.files.iter().map(|file| file.sha256.as_str()))
        .collect();
    let Ok(entries) = fs::read_dir(cache.join(FILES)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !used.contains(name.to_string_lossy().as_ref()) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(format!("{}: {error}", path.display()))
        }
        _ => Ok(()),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
