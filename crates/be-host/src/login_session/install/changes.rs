use std::error::Error;
use std::io::{self, IsTerminal, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
    Copy {
        source: PathBuf,
        files: Vec<PathBuf>,
        home: PathBuf,
    },
    Link {
        link: PathBuf,
        target: PathBuf,
    },
    Write {
        path: PathBuf,
        contents: String,
    },
    Remove {
        path: PathBuf,
    },
    Clear {
        home: PathBuf,
        files: Vec<PathBuf>,
    },
}

impl Change {
    fn destinations(&self) -> Vec<PathBuf> {
        match self {
            Change::Copy { home, .. } | Change::Clear { home, .. } => {
                vec![parent(home), home.clone()]
            }
            Change::Link { link: path, .. } | Change::Remove { path } => vec![parent(path)],
            Change::Write { path, .. } => vec![path.clone()],
        }
    }

    pub(crate) fn blocked(&self, access: &dyn Access) -> Option<PathBuf> {
        self.destinations()
            .into_iter()
            .find(|destination| !access.writable(destination))
    }

    fn whole(&self) -> bool {
        matches!(
            self,
            Change::Copy { .. } | Change::Link { .. } | Change::Clear { .. }
        )
    }

    fn apply(&self, staged: Option<PathBuf>) -> io::Result<()> {
        match self {
            Change::Copy {
                source,
                files,
                home,
            } => {
                let staged = match staged {
                    Some(staged) => staged,
                    None => stage(source, files, home)?,
                };
                commit(&staged, home)
            }
            Change::Link { link, target } => {
                std::fs::create_dir_all(parent(link))?;
                if std::fs::symlink_metadata(link).is_ok() {
                    std::fs::remove_file(link)?;
                }
                std::os::unix::fs::symlink(target, link)
            }
            Change::Write { path, contents } => {
                std::fs::create_dir_all(parent(path))?;
                std::fs::write(path, contents)
            }
            Change::Remove { path } => match std::fs::remove_file(path) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            },
            Change::Clear { home, files } => {
                let mut dirs = Vec::new();
                for file in files {
                    match std::fs::remove_file(home.join(file)) {
                        Err(error) if error.kind() != io::ErrorKind::NotFound => {
                            return Err(error);
                        }
                        _ => {}
                    }
                    let mut dir = file.parent();
                    while let Some(inner) = dir.filter(|dir| !dir.as_os_str().is_empty()) {
                        dirs.push(home.join(inner));
                        dir = inner.parent();
                    }
                }
                dirs.sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
                dirs.dedup();
                for dir in dirs.into_iter().chain([home.clone()]) {
                    let _ = std::fs::remove_dir(dir);
                }
                Ok(())
            }
        }
    }
}

fn parent(path: &Path) -> PathBuf {
    path.parent().unwrap_or(Path::new("/")).to_path_buf()
}

fn staging(home: &Path) -> (PathBuf, PathBuf) {
    (home.with_extension("new"), home.with_extension("old"))
}

fn stage(source: &Path, files: &[PathBuf], home: &Path) -> io::Result<PathBuf> {
    let (staged, old) = staging(home);
    for leftover in [&staged, &old] {
        if leftover.exists() {
            std::fs::remove_dir_all(leftover)?;
        }
    }
    let copied = files.iter().try_for_each(|file| {
        let target = staged.join(file);
        std::fs::create_dir_all(parent(&target))?;
        std::fs::copy(source.join(file), target).map(|_| ())
    });
    if let Err(error) = copied {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(error);
    }
    Ok(staged)
}

fn commit(staged: &Path, home: &Path) -> io::Result<()> {
    let (_, old) = staging(home);
    let kept = home.exists();
    if kept {
        std::fs::rename(home, &old)?;
    }
    if let Err(error) = std::fs::rename(staged, home) {
        if kept {
            let _ = std::fs::rename(&old, home);
        }
        return Err(error);
    }
    if kept {
        std::fs::remove_dir_all(&old)?;
    }
    Ok(())
}

pub(crate) fn apply(
    changes: &[Change],
    elevated: &[bool],
    elevate: impl FnOnce(&[Change]) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let elevated = |index: usize| elevated.get(index).copied().unwrap_or(false);
    let mut staged: Vec<Option<PathBuf>> = vec![None; changes.len()];
    for (index, change) in changes.iter().enumerate() {
        if let Change::Copy {
            source,
            files,
            home,
        } = change
            && !elevated(index)
        {
            match stage(source, files, home) {
                Ok(dir) => staged[index] = Some(dir),
                Err(error) => {
                    discard(&staged);
                    return Err(format!("copying into {} failed: {error}", home.display()).into());
                }
            }
        }
    }
    let theirs: Vec<Change> = changes
        .iter()
        .enumerate()
        .filter(|(index, _)| elevated(*index))
        .map(|(_, change)| change.clone())
        .collect();
    if !theirs.is_empty()
        && let Err(error) = elevate(&theirs)
    {
        discard(&staged);
        return Err(error);
    }
    for (index, change) in changes.iter().enumerate() {
        if !elevated(index) {
            change.apply(staged[index].take()).map_err(|error| {
                format!("{}: {error}", change.destinations()[0].display())
            })?;
        }
    }
    Ok(())
}

fn discard(staged: &[Option<PathBuf>]) {
    for dir in staged.iter().flatten() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

pub(crate) trait Access {
    fn writable(&self, path: &Path) -> bool;
    fn owner(&self, path: &Path) -> Option<u32>;
}

pub(crate) struct Here;

fn existing(path: &Path) -> Option<&Path> {
    path.ancestors()
        .find(|ancestor| std::fs::symlink_metadata(ancestor).is_ok())
}

impl Access for Here {
    fn writable(&self, path: &Path) -> bool {
        let Some(path) = existing(path) else {
            return false;
        };
        let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // unsafe(ffi)
        unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
    }

    fn owner(&self, path: &Path) -> Option<u32> {
        Some(std::fs::symlink_metadata(existing(path)?).ok()?.uid())
    }
}

pub(crate) struct User {
    pub(crate) root: bool,
    pub(crate) home: Option<PathBuf>,
}

impl User {
    pub(crate) fn current() -> Self {
        Self {
            // unsafe(ffi)
            root: unsafe { libc::geteuid() } == 0,
            home: std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|home| home.is_absolute() && home != Path::new("/")),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Here,
    Elevate { whole: bool, blocked: Vec<PathBuf> },
}

pub(crate) fn decide(
    changes: &[Change],
    access: &dyn Access,
    user: &User,
) -> Result<Decision, String> {
    if user.root {
        for destination in changes.iter().flat_map(Change::destinations) {
            if let Some(owner) = access.owner(&destination)
                && owner != 0
            {
                return Err(format!(
                    "{} belongs to the user with id {owner}, so root does not write there: run it as that user",
                    destination.display()
                ));
            }
        }
        return Ok(Decision::Here);
    }
    let mut blocked = Vec::new();
    let mut whole = false;
    for change in changes {
        let Some(path) = change.blocked(access) else {
            continue;
        };
        if let Some(home) = &user.home
            && path.starts_with(home)
        {
            return Err(format!(
                "you cannot write {}, and it is in your home directory, where it would be left owned by root: make it yours or pick another prefix",
                path.display()
            ));
        }
        whole |= change.whole();
        if !blocked.contains(&path) {
            blocked.push(path);
        }
    }
    Ok(match blocked.is_empty() {
        true => Decision::Here,
        false => Decision::Elevate { whole, blocked },
    })
}

pub(crate) fn list(paths: &[PathBuf]) -> String {
    let paths: Vec<String> = paths.iter().map(|path| path.display().to_string()).collect();
    match paths.as_slice() {
        [] => String::new(),
        [one] => format!("Writing {one}"),
        [rest @ .., last] => format!("Writing {} and {last}", rest.join(", ")),
    }
}

pub(crate) fn sudo_can_ask(blocked: &[PathBuf]) -> Result<(), String> {
    let quiet = Command::new("sudo")
        .args(["-n", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match quiet {
        Ok(status) if status.success() => Ok(()),
        Ok(_) if io::stdin().is_terminal() => Ok(()),
        Ok(_) => Err(format!(
            "{} needs root, and sudo cannot ask for a password without a terminal: run it in a terminal, or as root",
            list(blocked)
        )),
        Err(_) => Err(format!(
            "{} needs root, and there is no sudo: run it as root",
            list(blocked)
        )),
    }
}

pub(crate) fn sudo_again(arguments: &[&str]) -> Result<(), Box<dyn Error>> {
    let status = Command::new("sudo")
        .arg(std::env::current_exe()?)
        .args(arguments)
        .status()?;
    match status.success() {
        true => Ok(()),
        false => Err(format!("running it as root with sudo failed: {status}").into()),
    }
}

pub(crate) fn sudo_apply(changes: &[Change]) -> Result<(), Box<dyn Error>> {
    let mut child = Command::new("sudo")
        .arg(std::env::current_exe()?)
        .arg(crate::arguments::APPLY_CHANGES)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(mut input) = child.stdin.take() {
        input.write_all(to_json(changes)?.as_bytes())?;
    }
    let status = child.wait()?;
    match status.success() {
        true => Ok(()),
        false => Err(format!("writing as root with sudo failed: {status}").into()),
    }
}

pub(crate) fn apply_elevated() -> Result<(), Box<dyn Error>> {
    let mut text = String::new();
    io::stdin().read_to_string(&mut text)?;
    let changes = from_json(&text).ok_or("the changes to write as root are not readable")?;
    match decide(&changes, &Here, &User::current())? {
        Decision::Here => apply(&changes, &[], |_| Ok(())),
        Decision::Elevate { blocked, .. } => {
            Err(format!("{} needs root", list(&blocked)).into())
        }
    }
}

fn text(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{} is not UTF-8", path.display()))
}

pub(crate) fn to_json(changes: &[Change]) -> Result<String, String> {
    let mut values = Vec::new();
    for change in changes {
        values.push(match change {
            Change::Write { path, contents } => {
                serde_json::json!({ "write": text(path)?, "contents": contents })
            }
            Change::Remove { path } => serde_json::json!({ "remove": text(path)? }),
            _ => return Err("only files are written as root on their own".to_owned()),
        });
    }
    Ok(serde_json::Value::Array(values).to_string())
}

pub(crate) fn from_json(text: &str) -> Option<Vec<Change>> {
    let values: Vec<serde_json::Value> = serde_json::from_str(text).ok()?;
    values
        .iter()
        .map(|value| {
            if let Some(path) = value["write"].as_str() {
                return Some(Change::Write {
                    path: PathBuf::from(path),
                    contents: value["contents"].as_str()?.to_owned(),
                });
            }
            Some(Change::Remove {
                path: PathBuf::from(value["remove"].as_str()?),
            })
        })
        .collect()
}
