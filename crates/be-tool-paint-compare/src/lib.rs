//! The action that follows a painting test's (run_test in buck/dev/test.bzl):
//!
//! ```text
//! paint-compare --made DIR --accepted DIR --out OUT [--record DIR] [--update]
//! ```
//!
//! The test wrote every painting it made into `--made` and read none
//! (MADE_PAINTINGS, guides/testing_a_gui.md); `--accepted` holds the crate's
//! snapshots/. So the test's action is keyed on its code alone, and this one,
//! which is cheap, on what the test made and what is accepted: a test that
//! paints differently from run to run gives one painting per build, from the
//! cache, rather than one that flips with the accepted painting it read.
//!
//! For each painting made it writes, as the test once did itself:
//!
//! - OUT/used/NAME: the SHA-256 of the accepted painting it compared with,
//!   that of nothing when there was none, so that ./scripts/verify writes a
//!   painting only where the checkout still holds what it was compared with;
//! - under `--update`, OUT/changed/NAME and OUT/changed/NAME.why, the painting
//!   and what changed, when it is new or differs from the accepted one.
//!
//! `--record DIR` is a run_test "record" (crates/be-tool-run-test's
//! `--record`): its log and status are copied into OUT, and without
//! `--update` a painting that differs is said at the end of the log and makes
//! a status of 0 one of 1, as the test itself would have failed.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const USAGE: &str =
    "usage: paint-compare --made DIR --accepted DIR --out OUT [--record DIR] [--update]";

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub made: PathBuf,
    pub accepted: PathBuf,
    pub out: PathBuf,
    pub record: Option<PathBuf>,
    pub update: bool,
}

pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let (mut made, mut accepted, mut out) = (None, None, None);
    let mut options = Options::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let flag = argument.to_string_lossy().into_owned();
        let mut value = || {
            arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))
        };
        match flag.as_str() {
            "--made" => made = Some(value()?),
            "--accepted" => accepted = Some(value()?),
            "--out" => out = Some(value()?),
            "--record" => options.record = Some(value()?),
            "--update" => options.update = true,
            _ => return Err(format!("unknown argument {flag}\n{USAGE}")),
        }
    }
    let missing = |name: &str| format!("{name} is needed\n{USAGE}");
    options.made = made.ok_or_else(|| missing("--made"))?;
    options.accepted = accepted.ok_or_else(|| missing("--accepted"))?;
    options.out = out.ok_or_else(|| missing("--out"))?;
    Ok(options)
}

pub fn run(options: &Options) -> Result<(), String> {
    let (used, changed) = (options.out.join("used"), options.out.join("changed"));
    create_dir(&used)?;
    create_dir(&changed)?;
    let mut differences = Vec::new();
    for made_path in paintings(&options.made)? {
        let Some(name) = made_path.file_name() else {
            continue;
        };
        let made = read(&made_path)?;
        let accepted = std::fs::read(options.accepted.join(name)).ok();
        write(
            &used.join(name),
            sha256(accepted.as_deref().unwrap_or_default()).as_bytes(),
        )?;
        let why = match &accepted {
            None => "there is no accepted painting".to_owned(),
            Some(accepted) => match beui_paint_snapshot::changed(accepted, &made) {
                Some(why) => why,
                None => continue,
            },
        };
        if options.update {
            let mut why_name = name.to_owned();
            why_name.push(".why");
            write(&changed.join(name), &made)?;
            write(&changed.join(why_name), why.as_bytes())?;
        } else {
            differences.push(format!(
                "the painting {} changed: {why}\nto accept it:\n  {}",
                name.to_string_lossy(),
                beui_paint_snapshot::HOW_TO_ACCEPT
            ));
        }
    }
    let Some(record) = &options.record else {
        if differences.is_empty() {
            return Ok(());
        }
        return Err(differences.join("\n"));
    };
    let mut log = std::fs::read(record.join("log")).unwrap_or_default();
    let mut status = std::fs::read_to_string(record.join("status"))
        .map_err(|error| format!("cannot read {}: {error}", record.join("status").display()))?;
    for difference in &differences {
        log.extend_from_slice(difference.as_bytes());
        log.push(b'\n');
    }
    if !differences.is_empty() && status.trim() == "0" {
        status = "1\n".to_owned();
    }
    write(&options.out.join("log"), &log)?;
    write(&options.out.join("status"), status.as_bytes())
}

/// The paintings in a directory, by name; none when it does not exist.
fn paintings(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("cannot read {}: {error}", directory.display())),
    };
    let mut paintings: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "paint")
        })
        .collect();
    paintings.sort();
    Ok(paintings)
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn create_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests;
