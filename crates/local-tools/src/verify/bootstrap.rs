use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const PINS: &str = "scripts/internal/pins";
const CRATE: &str = "crates/bootstrap";

pub enum Outcome {
    Current,
    Published(Vec<String>),
    Stale,
}

pub fn check(built: Option<&Path>, out: Option<&Path>, check: bool) -> Result<Outcome, String> {
    let pins =
        fs::read_to_string(PINS).map_err(|error| format!("could not read {PINS}: {error}"))?;
    let source = source_hash(Path::new(CRATE))?;
    if pinned(&pins, "bootstrap_source") == Some(source.as_str()) {
        return Ok(Outcome::Current);
    }
    let (Some(built), Some(out), false) = (built, out, check) else {
        return Ok(Outcome::Stale);
    };
    fs::create_dir_all(out)
        .map_err(|error| format!("could not create {}: {error}", out.display()))?;
    let mut values = vec![("bootstrap_source".to_owned(), source)];
    let mut triples: Vec<PathBuf> = fs::read_dir(built)
        .map_err(|error| format!("could not read {}: {error}", built.display()))?
        .flatten()
        .map(|entry| entry.path())
        .collect();
    triples.sort();
    for directory in triples {
        let triple = directory
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let binary = ["bootstrap", "bootstrap.exe"]
            .iter()
            .map(|name| directory.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| format!("{} holds no bootstrap", directory.display()))?;
        let bytes = fs::read(&binary)
            .map_err(|error| format!("could not read {}: {error}", binary.display()))?;
        let sha256 = hex(&Sha256::digest(&bytes));
        let uploaded = out.join(&sha256);
        fs::write(&uploaded, &bytes)
            .map_err(|error| format!("could not write {}: {error}", uploaded.display()))?;
        values.push((
            format!("bootstrap_sha256_{}", triple.replace('-', "_")),
            sha256,
        ));
    }
    let written = set(&pins, &values)?;
    fs::write(PINS, written).map_err(|error| format!("could not write {PINS}: {error}"))?;
    Ok(Outcome::Published(
        values
            .into_iter()
            .map(|(name, value)| format!("{name}='{value}'"))
            .collect(),
    ))
}

fn pinned<'a>(pins: &'a str, name: &str) -> Option<&'a str> {
    pins.lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
        .map(|value| value.trim().trim_matches('\''))
}

pub fn set(pins: &str, values: &[(String, String)]) -> Result<String, String> {
    let mut lines: Vec<String> = pins.lines().map(str::to_owned).collect();
    for (name, value) in values {
        let line = lines
            .iter_mut()
            .find(|line| line.starts_with(&format!("{name}=")))
            .ok_or_else(|| format!("{PINS} has no {name}= line"))?;
        *line = format!("{name}='{value}'");
    }
    let mut written = lines.join("\n");
    if pins.ends_with('\n') {
        written.push('\n');
    }
    Ok(written)
}

pub fn source_hash(root: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort();
    let mut hasher = Sha256::new();
    for path in files {
        let bytes = fs::read(root.join(&path))
            .map_err(|error| format!("could not read {path}: {error}"))?;
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(hex(&Sha256::digest(&bytes)).as_bytes());
        hasher.update(b"\n");
    }
    Ok(hex(&hasher.finalize()))
}

fn collect(root: &Path, directory: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
