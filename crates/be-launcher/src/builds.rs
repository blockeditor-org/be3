use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const STORE: &str = "https://be3-ci.b-cdn.net/android";
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
        format!("{STORE}/{}/{hash}.apk", self.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Object {
    pub(crate) hash: String,
    pub(crate) size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Build {
    pub(crate) commit: String,
    pub(crate) app: Object,
    pub(crate) launcher: Object,
}

impl Build {
    pub(crate) fn parse(document: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(document)
            .map_err(|error| format!("be3-ci described the build as {error}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Installed {
    pub(crate) slot: Slot,
    pub(crate) commit: String,
    pub(crate) hash: String,
}

pub(crate) trait Fetch {
    fn fetch(&mut self, url: &str, into: &mut dyn Write) -> Result<(), String>;
}

pub(crate) fn installed(record: &Path) -> Option<Installed> {
    let document = fs::read(record).ok()?;
    serde_json::from_slice(&document).ok()
}

pub(crate) fn remember(record: &Path, installed: Option<&Installed>) -> Result<(), String> {
    match installed {
        Some(installed) => {
            let document = serde_json::to_vec(installed).map_err(|error| error.to_string())?;
            fs::write(record, document).map_err(|error| error.to_string())
        }
        None => remove(record),
    }
}

pub(crate) fn fetch_apk(
    path: &Path,
    slot: Slot,
    object: &Object,
    fetch: &mut dyn Fetch,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    if hash_file(path).as_deref() != Some(object.hash.as_str()) {
        let mut report = |written: u64| progress(written, object.size);
        download(
            fetch,
            &slot.object_url(&object.hash),
            &object.hash,
            path,
            &mut report,
        )?;
    }
    progress(object.size, object.size);
    Ok(())
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
    let mut hashing = Hashing {
        file,
        hasher: Sha256::new(),
        written: 0,
        progress,
    };
    let fetched = fetch.fetch(url, &mut hashing);
    let Hashing { file, hasher, .. } = hashing;
    if let Err(error) = fetched {
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
