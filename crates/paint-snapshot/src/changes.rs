use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::compare::frames;
use crate::{Snapshot, differences};

const FOLDER: &str = "snapshots";

pub struct Change {
    pub name: String,
    pub before: Option<Snapshot>,
    pub after: Option<Snapshot>,
}

impl Change {
    pub fn reasons(&self) -> Vec<String> {
        match (&self.before, &self.after) {
            (None, None) => Vec::new(),
            (None, Some(after)) => vec![format!("was added, {}", frames(after.frames.len()))],
            (Some(_), None) => vec!["was removed".to_owned()],
            (Some(before), Some(after)) => {
                let reasons: Vec<String> = differences(before, after)
                    .into_iter()
                    .map(|difference| difference.description)
                    .collect();
                match reasons.is_empty() {
                    true => {
                        vec!["the encoded painting changed, though it looks the same".to_owned()]
                    }
                    false => reasons,
                }
            }
        }
    }
}

pub fn root() -> Result<PathBuf, String> {
    let output = git(&["rev-parse", "--show-toplevel"])?;
    let text = String::from_utf8(output).map_err(|error| error.to_string())?;
    Ok(PathBuf::from(text.trim()))
}

pub fn changes(root: &Path, base: &str) -> Result<Vec<Change>, String> {
    let tracked = git_in(
        root,
        &["diff", "--no-renames", "--name-only", base, "--", FOLDER],
    )?;
    let untracked = git_in(
        root,
        &["ls-files", "--others", "--exclude-standard", "--", FOLDER],
    )?;
    let paths: BTreeSet<String> = [tracked, untracked]
        .iter()
        .flat_map(|output| {
            String::from_utf8_lossy(output)
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|path| path.ends_with(".paint"))
        .collect();

    paths
        .into_iter()
        .map(|path| {
            let name = Path::new(&path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let before = match git_in(root, &["cat-file", "-e", &format!("{base}:{path}")]) {
                Ok(_) => Some(decode(
                    &path,
                    &git_in(root, &["show", &format!("{base}:{path}")])?,
                )?),
                Err(_) => None,
            };
            let after = match std::fs::read(root.join(&path)) {
                Ok(bytes) => Some(decode(&path, &bytes)?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(format!("could not read {path}: {error}")),
            };
            Ok(Change {
                name,
                before,
                after,
            })
        })
        .collect()
}

fn decode(path: &str, bytes: &[u8]) -> Result<Snapshot, String> {
    Snapshot::decode(bytes).map_err(|error| format!("{path} is unreadable: {error}"))
}

fn git(arguments: &[&str]) -> Result<Vec<u8>, String> {
    run(Command::new("git").args(arguments), arguments)
}

fn git_in(root: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    run(
        Command::new("git").arg("-C").arg(root).args(arguments),
        arguments,
    )
}

fn run(command: &mut Command, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = command
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}
