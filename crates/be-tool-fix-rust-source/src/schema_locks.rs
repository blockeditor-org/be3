//! Each crate's `schema.lock`, rewritten from its `Schema` types (be-wasm-schema-lock): new
//! members numbered, removed ones retired, and a change a frozen type cannot take refused.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use be_wasm_schema_lock::{FILE, Lock, Source, update};

use crate::{collect_rust_files, relative};

/// A crate's lockfile: where it is, what it holds, and what it should hold (`None`: no file,
/// as the crate has no `Schema` types).
struct Planned {
    path: PathBuf,
    old: Option<String>,
    new: Option<String>,
}

fn plan(root: &Path) -> Result<Vec<Planned>, Box<dyn Error>> {
    let crates = root.join("crates");
    if !crates.is_dir() {
        return Ok(Vec::new());
    }
    let mut directories: Vec<PathBuf> = fs::read_dir(&crates)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect();
    directories.sort();
    let mut planned = Vec::new();
    let mut failures = Vec::new();
    for directory in directories {
        let path = directory.join(FILE);
        let old = match fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let mut paths = Vec::new();
        collect_rust_files(&directory, &mut paths)?;
        paths.sort();
        let mut texts = Vec::new();
        for file in &paths {
            texts.push((relative(&directory, file), fs::read_to_string(file)?));
        }
        if old.is_none() && !texts.iter().any(|(_, text)| text.contains("Schema")) {
            continue;
        }
        let sources: Vec<Source> = texts
            .iter()
            .map(|(path, text)| Source { path, text })
            .collect();
        let shown = relative(root, &path);
        let result = Lock::parse(old.as_deref().unwrap_or_default())
            .map_err(|error| vec![error])
            .and_then(|old| update(&sources, &old));
        match result {
            Ok(lock) => planned.push(Planned {
                path,
                old,
                new: (!lock.types.is_empty()).then(|| lock.to_text()),
            }),
            Err(errors) => {
                failures.extend(errors.into_iter().map(|error| format!("{shown}: {error}")));
            }
        }
    }
    if !failures.is_empty() {
        return Err(format!(
            "Schema types changed in ways their lockfile refuses:\n{}",
            failures.join("\n")
        )
        .into());
    }
    Ok(planned)
}

/// The lockfiles that are not what the source makes them.
pub(crate) fn stale(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    Ok(plan(root)?
        .into_iter()
        .filter(|planned| planned.old != planned.new)
        .map(|planned| format!("schema lockfile: {}", relative(root, &planned.path)))
        .collect())
}

/// Rewrites every lockfile the source changed, and removes those of crates with no `Schema`
/// types left.
pub(crate) fn write(root: &Path) -> Result<(), Box<dyn Error>> {
    for planned in plan(root)? {
        match (&planned.old, &planned.new) {
            (old, Some(new)) if old.as_ref() != Some(new) => fs::write(&planned.path, new)?,
            (Some(_), None) => fs::remove_file(&planned.path)?,
            _ => {}
        }
    }
    Ok(())
}
