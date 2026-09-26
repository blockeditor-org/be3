use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use flate2::write::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const STORE: &str = "https://be3-ci.b-cdn.net/android";
const DOWNLOADED: &str = "build.json";
const PART: &str = "part";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Slot {
    PullRequest(u64),
    Main,
}

impl Slot {
    pub(crate) fn name(self) -> String {
        match self {
            Slot::PullRequest(number) => format!("pr-{number}"),
            Slot::Main => "main".to_owned(),
        }
    }

    pub(crate) fn manifest_url(self, commit: &str) -> String {
        format!("{STORE}/{}/build.json?commit={commit}", self.name())
    }

    pub(crate) fn object_url(self, hash: &str) -> String {
        format!("{STORE}/{}/{hash}.gz", self.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Object {
    pub(crate) hash: String,
    pub(crate) size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) path: String,
    pub(crate) hash: String,
    pub(crate) size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Build {
    pub(crate) commit: String,
    pub(crate) shell: String,
    pub(crate) launcher: Object,
    pub(crate) files: Vec<Entry>,
}

impl Build {
    pub(crate) fn parse(document: &[u8]) -> Result<Self, String> {
        let build: Build = serde_json::from_slice(document)
            .map_err(|error| format!("be3-ci described the build as {error}"))?;
        for entry in &build.files {
            if !is_relative(&entry.path) {
                return Err(format!(
                    "be3-ci named a file outside the build: {}",
                    entry.path
                ));
            }
        }
        Ok(build)
    }

    pub(crate) fn size(&self) -> u64 {
        self.files.iter().map(|entry| entry.size).sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Downloaded {
    pub(crate) slot: Slot,
    pub(crate) commit: String,
}

pub(crate) trait Fetch {
    fn fetch(&mut self, url: &str, into: &mut dyn Write) -> Result<(), String>;
}

pub(crate) fn downloaded(directory: &Path) -> Option<Downloaded> {
    let document = fs::read(directory.join(DOWNLOADED)).ok()?;
    serde_json::from_slice(&document).ok()
}

pub(crate) fn sync(
    directory: &Path,
    slot: Slot,
    build: &Build,
    fetch: &mut dyn Fetch,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    remove(&directory.join(DOWNLOADED))?;
    let total = build.size();
    let mut done = 0;
    for entry in &build.files {
        let target = directory.join(&entry.path);
        if hash_file(&target).as_deref() != Some(entry.hash.as_str()) {
            let mut report = |written: u64| progress(done + written, total);
            download(
                fetch,
                &slot.object_url(&entry.hash),
                &entry.hash,
                &target,
                &mut report,
            )?;
        }
        done += entry.size;
        progress(done, total);
    }
    let kept: HashSet<PathBuf> = build
        .files
        .iter()
        .map(|entry| directory.join(&entry.path))
        .collect();
    remove_stale(directory, &kept)?;
    let downloaded = Downloaded {
        slot,
        commit: build.commit.clone(),
    };
    let document = serde_json::to_vec(&downloaded).map_err(|error| error.to_string())?;
    fs::write(directory.join(DOWNLOADED), document).map_err(|error| error.to_string())
}

pub(crate) fn fetch_launcher(
    path: &Path,
    slot: Slot,
    build: &Build,
    fetch: &mut dyn Fetch,
) -> Result<(), String> {
    if hash_file(path).as_deref() == Some(build.launcher.hash.as_str()) {
        return Ok(());
    }
    download(
        fetch,
        &slot.object_url(&build.launcher.hash),
        &build.launcher.hash,
        path,
        &mut |_| {},
    )
}

fn download(
    fetch: &mut dyn Fetch,
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
    let mut decoder = GzDecoder::new(Hashing {
        file,
        hasher: Sha256::new(),
        written: 0,
        progress,
    });
    fetch.fetch(url, &mut decoder)?;
    let hashing = decoder
        .finish()
        .map_err(|error| format!("{url} is not gzip: {error}"))?;
    let received = hex(&hashing.hasher.finalize());
    if received != hash {
        let _ = fs::remove_file(&part);
        return Err(format!("{url} held {received}, not {hash}"));
    }
    hashing.file.sync_all().map_err(|error| error.to_string())?;
    drop(hashing.file);
    let mut permissions = fs::metadata(&part)
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&part, permissions).map_err(|error| error.to_string())?;
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

fn remove_stale(directory: &Path, kept: &HashSet<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory).map_err(|error| error.to_string())?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            remove_stale(&path, kept)?;
            if fs::read_dir(&path)
                .map_err(|error| error.to_string())?
                .next()
                .is_none()
            {
                fs::remove_dir(&path).map_err(|error| error.to_string())?;
            }
        } else if !kept.contains(&path) {
            remove(&path)?;
        }
    }
    Ok(())
}

fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(format!("{}: {error}", path.display()))
        }
        _ => Ok(()),
    }
}

fn hash_file(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_relative(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}
