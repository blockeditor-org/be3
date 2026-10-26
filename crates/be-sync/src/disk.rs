use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::index::Stat;
use crate::{ERRORS_FILE, IGNORE_FILE, STATE_DIRECTORY};

const BUILT_IN: [&str; 3] = [".git/", "target/", "buck-out/"];
const TEMPORARY: &str = "tmp";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Directory,
    File,
}

#[derive(Clone, Debug)]
pub struct Found {
    pub kind: Kind,
    pub executable: bool,
    pub stat: Stat,
}

#[derive(Default)]
pub struct Disk {
    pub found: BTreeMap<String, Found>,
    pub problems: Vec<String>,
    rules: Rules,
}

impl Disk {
    pub fn ignored(&self, path: &str, directory: bool) -> bool {
        self.rules.ignored(path, directory)
    }
}

#[derive(Default)]
struct Rules {
    built_in: Option<Gitignore>,
    by_directory: HashMap<String, Vec<Gitignore>>,
}

impl Rules {
    fn ignored(&self, path: &str, directory: bool) -> bool {
        if path == STATE_DIRECTORY || path == ERRORS_FILE {
            return true;
        }
        let mut level = parent_of(path);
        loop {
            for matcher in self.by_directory.get(level).into_iter().flatten() {
                match matcher.matched(path, directory) {
                    Match::Ignore(_) => return true,
                    Match::Whitelist(_) => return false,
                    Match::None => {}
                }
            }
            if level.is_empty() {
                break;
            }
            level = parent_of(level);
        }
        self.built_in
            .as_ref()
            .is_some_and(|built_in| built_in.matched(path, directory).is_ignore())
    }

    fn read(&mut self, root: &Path, directory: &str, problems: &mut Vec<String>) {
        let mut matchers = Vec::new();
        for name in [IGNORE_FILE, ".gitignore"] {
            let file = root.join(join(directory, name));
            if !file.is_file() {
                continue;
            }
            let mut builder = GitignoreBuilder::new(directory);
            if let Some(error) = builder.add(&file) {
                problems.push(format!("{}: {error}", join(directory, name)));
            }
            match builder.build() {
                Ok(matcher) => matchers.push(matcher),
                Err(error) => problems.push(format!("{}: {error}", join(directory, name))),
            }
        }
        if !matchers.is_empty() {
            self.by_directory.insert(directory.to_owned(), matchers);
        }
    }
}

pub fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

pub fn name_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

pub fn join(parent: &str, name: &str) -> String {
    match parent.is_empty() {
        true => name.to_owned(),
        false => format!("{parent}/{name}"),
    }
}

pub fn walk(root: &Path) -> Result<Disk, String> {
    let mut disk = Disk::default();
    let mut built_in = GitignoreBuilder::new("");
    for line in BUILT_IN {
        built_in
            .add_line(None, line)
            .map_err(|error| error.to_string())?;
    }
    disk.rules.built_in = Some(built_in.build().map_err(|error| error.to_string())?);
    let mut pending = vec![String::new()];
    while let Some(directory) = pending.pop() {
        disk.rules.read(root, &directory, &mut disk.problems);
        let absolute = root.join(&directory);
        let entries = std::fs::read_dir(&absolute)
            .map_err(|error| format!("could not list {}: {error}", absolute.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("could not list {}: {error}", absolute.display()))?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                disk.problems.push(format!(
                    "{} is not named in UTF-8, so it cannot be a block's name",
                    absolute.join(entry.file_name()).display()
                ));
                continue;
            };
            let path = join(&directory, &name);
            let metadata = entry
                .metadata()
                .map_err(|error| format!("could not read {path}: {error}"))?;
            let kind = match metadata.file_type() {
                kind if kind.is_dir() => Kind::Directory,
                kind if kind.is_file() => Kind::File,
                kind if kind.is_symlink() => {
                    if !disk.rules.ignored(&path, false) {
                        disk.problems.push(format!(
                            "{path} is a symbolic link, which be sync leaves alone"
                        ));
                    }
                    continue;
                }
                _ => continue,
            };
            if disk.rules.ignored(&path, kind == Kind::Directory) {
                continue;
            }
            if kind == Kind::Directory {
                pending.push(path.clone());
            }
            disk.found.insert(
                path,
                Found {
                    kind,
                    executable: executable(&metadata),
                    stat: stat_of(&metadata),
                },
            );
        }
    }
    Ok(disk)
}

#[cfg(unix)]
fn executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    false
}

pub fn stat_of(metadata: &std::fs::Metadata) -> Stat {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| {
            i64::try_from(since.as_nanos()).unwrap_or(i64::MAX)
        });
    Stat {
        size: metadata.len(),
        modified,
        inode: inode(metadata),
    }
}

#[cfg(unix)]
fn inode(metadata: &std::fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::ino(metadata)
}

#[cfg(not(unix))]
fn inode(_: &std::fs::Metadata) -> u64 {
    0
}

pub fn stat(path: &Path) -> Option<Stat> {
    std::fs::symlink_metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|metadata| stat_of(&metadata))
}

pub fn temporary(root: &Path) -> Result<PathBuf, String> {
    let directory = root.join(STATE_DIRECTORY).join(TEMPORARY);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not make {}: {error}", directory.display()))?;
    Ok(directory)
}

pub fn write_atomically(scratch: &Path, path: &Path, bytes: &[u8]) -> Result<(), String> {
    let staged = scratch.join(format!(".{}.part", uuid::Uuid::new_v4().simple()));
    let written = std::fs::File::create(&staged).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_data()
    });
    if let Err(error) = written.and_then(|()| std::fs::rename(&staged, path)) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("could not write {}: {error}", path.display()));
    }
    Ok(())
}

#[cfg(unix)]
pub fn set_executable(path: &Path, executable: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?
        .permissions();
    let mode = permissions.mode();
    let wanted = match executable {
        true => mode | ((mode & 0o444) >> 2),
        false => mode & !0o111,
    };
    if wanted != mode {
        permissions.set_mode(wanted);
        std::fs::set_permissions(path, permissions)
            .map_err(|error| format!("could not change {}: {error}", path.display()))?;
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn set_executable(_: &Path, _: bool) -> Result<(), String> {
    Ok(())
}

const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

const FORBIDDEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

const MAX_FILE_NAME_BYTES: usize = 255;

pub fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("it has no name".to_owned());
    }
    if name == "." || name == ".." {
        return Some(format!("{name} names a directory itself"));
    }
    if let Some(character) = name
        .chars()
        .find(|character| FORBIDDEN.contains(character) || character.is_control())
    {
        return Some(match character {
            '/' | '\\' => format!("{character} separates directories in a path"),
            character if character.is_control() => {
                format!(
                    "it holds the control character U+{:04X}",
                    u32::from(character)
                )
            }
            character => format!("{character} is not allowed in a file name on Windows"),
        });
    }
    if name.ends_with(['.', ' ']) {
        return Some("Windows drops a dot or space at the end of a file name".to_owned());
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let upper = stem.to_ascii_uppercase();
    let numbered = ["COM", "LPT"].iter().any(|prefix| {
        upper.len() == 4 && upper.starts_with(prefix) && upper.as_bytes()[3].is_ascii_digit()
    });
    if RESERVED.contains(&upper.as_str()) || numbered {
        return Some(format!("{stem} is a device name on Windows"));
    }
    if name.len() > MAX_FILE_NAME_BYTES {
        return Some(format!(
            "it is longer than the {MAX_FILE_NAME_BYTES} bytes a file name may be"
        ));
    }
    None
}
