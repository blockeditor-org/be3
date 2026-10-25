use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use be_types_host::PlainFile;
use uuid::Uuid;

const RECORD: &str = ".written";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    len: u64,
    modified: u128,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OnDisk {
    pub(crate) path: PathBuf,
    pub(crate) stamp: Stamp,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Placed {
    pub(crate) disk: OnDisk,
    pub(crate) taken_back: Option<(String, Vec<u8>)>,
}

pub(crate) fn file_name(file: &PlainFile) -> String {
    let name = file.name.as_deref().unwrap_or_default();
    let name = be_block::plain::export_name(name, &file.extension);
    match name.starts_with('.') {
        true => format!("_{name}"),
        false => name,
    }
}

pub(crate) fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(Stamp {
        len: metadata.len(),
        modified,
    })
}

fn recorded(dir: &Path, name: &str) -> Option<Stamp> {
    let text = std::fs::read_to_string(dir.join(RECORD)).ok()?;
    let (stamp, recorded_name) = text.split_once('\n')?;
    if recorded_name != name {
        return None;
    }
    let (len, modified) = stamp.split_once(' ')?;
    Some(Stamp {
        len: len.parse().ok()?,
        modified: modified.parse().ok()?,
    })
}

pub(crate) fn record(path: &Path, stamp: Stamp) -> io::Result<()> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(io::Error::other("the file has no folder"));
    };
    std::fs::write(
        dir.join(RECORD),
        format!(
            "{} {}\n{}",
            stamp.len,
            stamp.modified,
            name.to_string_lossy()
        ),
    )
}

fn write(path: &Path, bytes: &[u8]) -> io::Result<Stamp> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("the file has no folder"))?;
    let partial = dir.join(".writing");
    std::fs::write(&partial, bytes)?;
    std::fs::rename(&partial, path)?;
    let stamp = stamp(path).ok_or_else(|| io::Error::other("the written file cannot be read"))?;
    record(path, stamp)?;
    Ok(stamp)
}

pub(crate) fn place(root: &Path, block: Uuid, file: &PlainFile) -> Result<Placed, String> {
    let dir = root.join(block.to_string());
    let name = file_name(file);
    let path = dir.join(&name);
    let placed = || -> io::Result<Placed> {
        std::fs::create_dir_all(&dir)?;
        let before = recorded(&dir, &name);
        let on_disk = stamp(&path);
        if let (Some(before), Some(on_disk)) = (before, on_disk)
            && before != on_disk
        {
            let bytes = std::fs::read(&path)?;
            record(&path, on_disk)?;
            let taken_back = (bytes != file.bytes).then(|| (name.clone(), bytes));
            return Ok(Placed {
                disk: OnDisk {
                    path: path.clone(),
                    stamp: on_disk,
                },
                taken_back,
            });
        }
        let stamp = match on_disk {
            Some(on_disk) if std::fs::read(&path)? == file.bytes => {
                record(&path, on_disk)?;
                on_disk
            }
            _ => write(&path, &file.bytes)?,
        };
        Ok(Placed {
            disk: OnDisk {
                path: path.clone(),
                stamp,
            },
            taken_back: None,
        })
    };
    placed().map_err(|error| format!("{} could not be written: {error}", path.display()))
}

pub(crate) fn changed(disk: &OnDisk) -> Option<(String, Vec<u8>, Stamp)> {
    let now = stamp(&disk.path)?;
    if now == disk.stamp {
        return None;
    }
    let bytes = std::fs::read(&disk.path).ok()?;
    let name = disk.path.file_name()?.to_string_lossy().into_owned();
    Some((name, bytes, now))
}

#[cfg(test)]
mod tests;
