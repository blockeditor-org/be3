//! WASI preview1, natively.
//!
//! An app gets what crates/be-host/web/wasi.js gives it in the browser: no files, arguments or
//! environment; standard error to the host's, standard output dropped and standard input empty;
//! the clocks, randomness, yielding and sleeping on a clock; and exiting, which stops it. A test
//! run (harness.rs) also gets its arguments, the host's environment, its standard output, and the
//! directories it is given, each opened at its own path, read and written through `std::fs`. A
//! path that leaves its directory through `..` is refused; one that leaves it through a symbolic
//! link is not, so a run is only as confined as the directories it is handed. Any call not here is
//! unsupported (ENOSYS). A pure guest gets none of them.

use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, PoisonError},
    time::{Duration, Instant, SystemTime},
};

use crate::engine::Defined;
use crate::gpu::throw;
use crate::state::{View, state_of};

pub(crate) const MODULE: &str = "wasi_snapshot_preview1";

const SUCCESS: i32 = 0;
const EACCES: i32 = 2;
const EBADF: i32 = 8;
const EEXIST: i32 = 20;
const EINVAL: i32 = 28;
const EIO: i32 = 29;
const EISDIR: i32 = 31;
const ENOENT: i32 = 44;
const ENOSYS: i32 = 52;
const ENOTDIR: i32 = 54;
const ENOTEMPTY: i32 = 55;
const ESPIPE: i32 = 70;
const ENOTCAPABLE: i32 = 76;

const STDIN: u32 = 0;
const STDOUT: u32 = 1;
const STDERR: u32 = 2;
const FILETYPE_CHARACTER_DEVICE: u8 = 2;
const FILETYPE_DIRECTORY: u8 = 3;
const FILETYPE_REGULAR_FILE: u8 = 4;
const FILETYPE_SYMBOLIC_LINK: u8 = 7;
const REALTIME: u32 = 0;
const SUBSCRIPTION: u32 = 48;
const EVENT: u32 = 32;
const CLOCK: u8 = 0;
const ABSOLUTE: u16 = 1;
const CREATE: u32 = 1;
const DIRECTORY: u32 = 2;
const EXCLUSIVE: u32 = 4;
const TRUNCATE: u32 = 8;
const APPEND: u32 = 1;
const RIGHT_READ: u64 = 1 << 1;
const RIGHT_WRITE: u64 = 1 << 6;
const FOLLOW: u32 = 1;
const DIRENT: usize = 24;

/// Where a stream's bytes go.
#[derive(Clone, Default)]
pub(crate) enum Output {
    #[default]
    Dropped,
    Inherited,
    Kept(Arc<Mutex<Vec<u8>>>),
    /// Kept, and written to the host's standard error unless held.
    Tee { kept: Arc<Mutex<Vec<u8>>>, held: bool },
}

impl Output {
    fn write(&self, stdout: bool, bytes: &[u8]) {
        let echo = |bytes: &[u8]| match stdout {
            true => {
                let mut out = io::stdout().lock();
                let _ = out.write_all(bytes);
                let _ = out.flush();
            }
            false => {
                let mut out = io::stderr().lock();
                let _ = out.write_all(bytes);
                let _ = out.flush();
            }
        };
        match self {
            Self::Dropped => {}
            Self::Inherited => echo(bytes),
            Self::Kept(kept) => kept
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(bytes),
            Self::Tee { kept, held } => {
                if !held {
                    echo(bytes);
                }
                kept.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend_from_slice(bytes);
            }
        }
    }

    /// What it kept, as text.
    pub(crate) fn contents(&self) -> String {
        match self {
            Self::Kept(kept) | Self::Tee { kept, .. } => {
                String::from_utf8_lossy(&kept.lock().unwrap_or_else(PoisonError::into_inner))
                    .into_owned()
            }
            Self::Dropped | Self::Inherited => String::new(),
        }
    }
}

enum Open {
    Directory(PathBuf),
    File { file: File, append: bool },
}

/// What a guest's WASI calls reach: the same for every thread of one guest.
#[derive(Default)]
pub(crate) struct Wasi {
    arguments: Vec<String>,
    environment: Vec<String>,
    stdout: Output,
    stderr: Output,
    /// Each preopened directory's descriptor and the name the guest knows it by.
    preopened: Vec<(u32, String)>,
    open: Mutex<HashMap<u32, Open>>,
    next: Mutex<u32>,
}

impl Wasi {
    /// An app's: standard error inherited, and nothing else.
    pub(crate) fn app() -> Arc<Self> {
        Arc::new(Self {
            stderr: Output::Inherited,
            ..Self::default()
        })
    }

    /// A test run's.
    pub(crate) fn run(
        arguments: &[String],
        stdout: Output,
        stderr: Output,
        directories: &[PathBuf],
    ) -> Result<Arc<Self>, String> {
        let mut open = HashMap::new();
        let mut preopened = Vec::new();
        for (index, directory) in directories.iter().enumerate() {
            let name = directory
                .to_str()
                .ok_or_else(|| format!("{} is not a usable path", directory.display()))?;
            if !directory.is_dir() {
                return Err(format!(
                    "{} could not be opened for the tests",
                    directory.display()
                ));
            }
            let fd = 3 + index as u32;
            open.insert(fd, Open::Directory(directory.clone()));
            preopened.push((fd, name.to_owned()));
        }
        Ok(Arc::new(Self {
            arguments: arguments.to_vec(),
            environment: std::env::vars()
                .map(|(name, value)| format!("{name}={value}"))
                .collect(),
            stdout,
            stderr,
            next: Mutex::new(3 + preopened.len() as u32),
            preopened,
            open: Mutex::new(open),
        }))
    }

    fn open(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Open>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn directory(&self, fd: u32) -> Result<PathBuf, i32> {
        match self.open().get(&fd) {
            Some(Open::Directory(path)) => Ok(path.clone()),
            Some(Open::File { .. }) => Err(ENOTDIR),
            None => Err(EBADF),
        }
    }

    fn add(&self, open: Open) -> u32 {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        let fd = *next;
        *next += 1;
        self.open().insert(fd, open);
        fd
    }
}

/// A file's device, inode and link count, where the platform says them.
#[cfg(unix)]
fn identity(metadata: &fs::Metadata) -> (u64, u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino(), metadata.nlink())
}

#[cfg(not(unix))]
fn identity(_: &fs::Metadata) -> (u64, u64, u64) {
    (0, 0, 1)
}

/// Reads at `at` without moving the file's position.
fn read_at(file: &mut File, bytes: &mut [u8], at: u64) -> io::Result<usize> {
    let position = file.stream_position()?;
    file.seek(SeekFrom::Start(at))?;
    let read = file.read(bytes);
    file.seek(SeekFrom::Start(position))?;
    read
}

/// Writes at `at` without moving the file's position.
fn write_at(file: &mut File, bytes: &[u8], at: u64) -> io::Result<()> {
    let position = file.stream_position()?;
    file.seek(SeekFrom::Start(at))?;
    let written = file.write_all(bytes);
    file.seek(SeekFrom::Start(position))?;
    written
}

fn errno(error: &io::Error) -> i32 {
    match error.kind() {
        io::ErrorKind::NotFound => ENOENT,
        io::ErrorKind::PermissionDenied => EACCES,
        io::ErrorKind::AlreadyExists => EEXIST,
        io::ErrorKind::DirectoryNotEmpty => ENOTEMPTY,
        io::ErrorKind::NotADirectory => ENOTDIR,
        io::ErrorKind::IsADirectory => EISDIR,
        io::ErrorKind::InvalidInput => EINVAL,
        _ => EIO,
    }
}

/// One call: its arguments as raw words, the memory, and the guest's WASI.
struct Call<'a> {
    arguments: Vec<u64>,
    view: View,
    wasi: &'a Wasi,
}

/// A call's outcome: an errno for the guest, or why the guest stops.
type Answer = Result<i32, String>;

impl Call<'_> {
    fn u32(&self, index: usize) -> u32 {
        self.arguments.get(index).copied().unwrap_or(0) as u32
    }

    fn u64(&self, index: usize) -> u64 {
        self.arguments.get(index).copied().unwrap_or(0)
    }

    fn write_u32(&self, at: u32, value: u32) -> Result<(), String> {
        self.view.write(at, &value.to_le_bytes())
    }

    fn write_u64(&self, at: u32, value: u64) -> Result<(), String> {
        self.view.write(at, &value.to_le_bytes())
    }

    fn read_u32(&self, at: u32) -> Result<u32, String> {
        let bytes = self.view.read(at, 4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&self, at: u32) -> Result<u64, String> {
        let mut word = [0; 8];
        word.copy_from_slice(&self.view.read(at, 8)?);
        Ok(u64::from_le_bytes(word))
    }

    /// The buffers an iovec array at `at` of `count` names.
    fn buffers(&self, at: u32, count: u32) -> Result<Vec<(u32, u32)>, String> {
        (0..count)
            .map(|index| {
                let entry = at.wrapping_add(index.wrapping_mul(8));
                Ok((self.read_u32(entry)?, self.read_u32(entry + 4)?))
            })
            .collect()
    }

    fn gathered(&self, at: u32, count: u32) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        for (pointer, length) in self.buffers(at, count)? {
            bytes.extend(self.view.read(pointer, length)?);
        }
        Ok(bytes)
    }

    fn scattered(&self, at: u32, count: u32, bytes: &[u8]) -> Result<u32, String> {
        let mut given = 0usize;
        for (pointer, length) in self.buffers(at, count)? {
            let take = (length as usize).min(bytes.len() - given);
            self.view.write(pointer, &bytes[given..given + take])?;
            given += take;
            if given == bytes.len() {
                break;
            }
        }
        Ok(given as u32)
    }

    fn text(&self, pointer: u32, length: u32) -> Result<String, String> {
        Ok(String::from_utf8_lossy(&self.view.read(pointer, length)?).into_owned())
    }

    /// The host path of `path` under the directory `fd`, refusing one that leaves it.
    fn path(&self, fd: u32, pointer: u32, length: u32) -> Result<Result<PathBuf, i32>, String> {
        let path = self.text(pointer, length)?;
        let directory = match self.wasi.directory(fd) {
            Ok(directory) => directory,
            Err(errno) => return Ok(Err(errno)),
        };
        let mut resolved = directory.clone();
        let mut depth = 0usize;
        for component in Path::new(&path).components() {
            match component {
                Component::Normal(part) => {
                    resolved.push(part);
                    depth += 1;
                }
                Component::CurDir => {}
                Component::ParentDir if depth > 0 => {
                    resolved.pop();
                    depth -= 1;
                }
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Ok(Err(ENOTCAPABLE));
                }
            }
        }
        Ok(Ok(resolved))
    }
}

macro_rules! errno {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(errno) => return Ok(errno),
        }
    };
}

fn io<T>(result: io::Result<T>) -> Result<T, i32> {
    result.map_err(|error| errno(&error))
}

fn stream(fd: u32) -> bool {
    matches!(fd, STDIN | STDOUT | STDERR)
}

fn fd_write(call: &Call) -> Answer {
    let (fd, written) = (call.u32(0), call.u32(3));
    let bytes = call.gathered(call.u32(1), call.u32(2))?;
    match fd {
        STDOUT => call.wasi.stdout.write(true, &bytes),
        STDERR => call.wasi.stderr.write(false, &bytes),
        STDIN => return Ok(EBADF),
        _ => {
            let mut open = call.wasi.open();
            match open.get_mut(&fd) {
                Some(Open::File { file, append }) => {
                    if *append {
                        errno!(io(file.seek(SeekFrom::End(0))));
                    }
                    errno!(io(file.write_all(&bytes)));
                }
                Some(Open::Directory(_)) => return Ok(EISDIR),
                None => return Ok(EBADF),
            }
        }
    }
    call.write_u32(written, bytes.len() as u32)?;
    Ok(SUCCESS)
}

fn fd_pwrite(call: &Call) -> Answer {
    let bytes = call.gathered(call.u32(1), call.u32(2))?;
    let mut open = call.wasi.open();
    let Some(Open::File { file, .. }) = open.get_mut(&call.u32(0)) else {
        return Ok(EBADF);
    };
    errno!(io(write_at(file, &bytes, call.u64(3))));
    call.write_u32(call.u32(4), bytes.len() as u32)?;
    Ok(SUCCESS)
}

fn capacity(call: &Call, at: u32, count: u32) -> Result<usize, String> {
    Ok(call
        .buffers(at, count)?
        .iter()
        .map(|(_, length)| *length as usize)
        .sum())
}

fn fd_read(call: &Call) -> Answer {
    let (fd, iovs, count, read) = (call.u32(0), call.u32(1), call.u32(2), call.u32(3));
    if fd == STDIN {
        call.write_u32(read, 0)?;
        return Ok(SUCCESS);
    }
    let mut bytes = vec![0; capacity(call, iovs, count)?];
    let got = {
        let mut open = call.wasi.open();
        match open.get_mut(&fd) {
            Some(Open::File { file, .. }) => errno!(io(file.read(&mut bytes))),
            Some(Open::Directory(_)) => return Ok(EISDIR),
            None => return Ok(EBADF),
        }
    };
    let given = call.scattered(iovs, count, &bytes[..got])?;
    call.write_u32(read, given)?;
    Ok(SUCCESS)
}

fn fd_pread(call: &Call) -> Answer {
    let (iovs, count) = (call.u32(1), call.u32(2));
    let mut bytes = vec![0; capacity(call, iovs, count)?];
    let got = {
        let mut open = call.wasi.open();
        let Some(Open::File { file, .. }) = open.get_mut(&call.u32(0)) else {
            return Ok(EBADF);
        };
        errno!(io(read_at(file, &mut bytes, call.u64(3))))
    };
    let given = call.scattered(iovs, count, &bytes[..got])?;
    call.write_u32(call.u32(4), given)?;
    Ok(SUCCESS)
}

fn fd_seek(call: &Call) -> Answer {
    let fd = call.u32(0);
    if stream(fd) {
        return Ok(ESPIPE);
    }
    let offset = call.u64(1) as i64;
    let from = match call.u32(2) {
        0 => SeekFrom::Start(offset as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return Ok(EINVAL),
    };
    let mut open = call.wasi.open();
    let Some(Open::File { file, .. }) = open.get_mut(&fd) else {
        return Ok(EBADF);
    };
    let at = errno!(io(file.seek(from)));
    drop(open);
    call.write_u64(call.u32(3), at)?;
    Ok(SUCCESS)
}

fn fd_tell(call: &Call) -> Answer {
    let mut open = call.wasi.open();
    let Some(Open::File { file, .. }) = open.get_mut(&call.u32(0)) else {
        return Ok(EBADF);
    };
    let at = errno!(io(file.stream_position()));
    drop(open);
    call.write_u64(call.u32(1), at)?;
    Ok(SUCCESS)
}

fn fd_close(call: &Call) -> Answer {
    let fd = call.u32(0);
    Ok(match stream(fd) || call.wasi.open().remove(&fd).is_some() {
        true => SUCCESS,
        false => EBADF,
    })
}

fn fd_sync(call: &Call) -> Answer {
    let fd = call.u32(0);
    if stream(fd) {
        return Ok(SUCCESS);
    }
    let open = call.wasi.open();
    Ok(match open.get(&fd) {
        Some(Open::File { file, .. }) => errno!(io(file.sync_all()).map(|()| SUCCESS)),
        Some(Open::Directory(_)) => SUCCESS,
        None => EBADF,
    })
}

fn fd_datasync(call: &Call) -> Answer {
    fd_sync(call)
}

fn fd_fdstat_get(call: &Call) -> Answer {
    let fd = call.u32(0);
    let (kind, flags) = match fd {
        _ if stream(fd) => (FILETYPE_CHARACTER_DEVICE, 0u16),
        _ => match call.wasi.open().get(&fd) {
            Some(Open::Directory(_)) => (FILETYPE_DIRECTORY, 0),
            Some(Open::File { append, .. }) => (FILETYPE_REGULAR_FILE, u16::from(*append)),
            None => return Ok(EBADF),
        },
    };
    let mut fdstat = [0u8; 24];
    fdstat[0] = kind;
    fdstat[2..4].copy_from_slice(&flags.to_le_bytes());
    fdstat[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    fdstat[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    call.view.write(call.u32(1), &fdstat)?;
    Ok(SUCCESS)
}

fn fd_fdstat_set_flags(call: &Call) -> Answer {
    let fd = call.u32(0);
    if stream(fd) {
        return Ok(SUCCESS);
    }
    let mut open = call.wasi.open();
    match open.get_mut(&fd) {
        Some(Open::File { append, .. }) => {
            *append = call.u32(1) & APPEND != 0;
            Ok(SUCCESS)
        }
        Some(Open::Directory(_)) => Ok(SUCCESS),
        None => Ok(EBADF),
    }
}

fn nanoseconds(time: io::Result<SystemTime>) -> u64 {
    time.ok()
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos() as u64)
}

fn filestat(call: &Call, at: u32, metadata: &fs::Metadata) -> Answer {
    let kind = match metadata.file_type() {
        kind if kind.is_dir() => FILETYPE_DIRECTORY,
        kind if kind.is_symlink() => FILETYPE_SYMBOLIC_LINK,
        kind if kind.is_file() => FILETYPE_REGULAR_FILE,
        _ => 0,
    };
    let (device, inode, links) = identity(metadata);
    let mut stat = [0u8; 64];
    stat[0..8].copy_from_slice(&device.to_le_bytes());
    stat[8..16].copy_from_slice(&inode.to_le_bytes());
    stat[16] = kind;
    stat[24..32].copy_from_slice(&links.to_le_bytes());
    stat[32..40].copy_from_slice(&metadata.len().to_le_bytes());
    stat[40..48].copy_from_slice(&nanoseconds(metadata.accessed()).to_le_bytes());
    stat[48..56].copy_from_slice(&nanoseconds(metadata.modified()).to_le_bytes());
    stat[56..64].copy_from_slice(&nanoseconds(metadata.modified()).to_le_bytes());
    call.view.write(at, &stat)?;
    Ok(SUCCESS)
}

fn fd_filestat_get(call: &Call) -> Answer {
    let fd = call.u32(0);
    let metadata = match call.wasi.open().get(&fd) {
        Some(Open::File { file, .. }) => errno!(io(file.metadata())),
        Some(Open::Directory(path)) => errno!(io(fs::metadata(path))),
        None => return Ok(EBADF),
    };
    filestat(call, call.u32(1), &metadata)
}

fn fd_filestat_set_size(call: &Call) -> Answer {
    let open = call.wasi.open();
    let Some(Open::File { file, .. }) = open.get(&call.u32(0)) else {
        return Ok(EBADF);
    };
    Ok(errno!(io(file.set_len(call.u64(1))).map(|()| SUCCESS)))
}

fn path_filestat_get(call: &Call) -> Answer {
    let path = errno!(call.path(call.u32(0), call.u32(2), call.u32(3))?);
    let metadata = match call.u32(1) & FOLLOW {
        0 => errno!(io(fs::symlink_metadata(path))),
        _ => errno!(io(fs::metadata(path))),
    };
    filestat(call, call.u32(4), &metadata)
}

fn fd_prestat_get(call: &Call) -> Answer {
    let fd = call.u32(0);
    let Some((_, name)) = call.wasi.preopened.iter().find(|(open, _)| *open == fd) else {
        return Ok(EBADF);
    };
    let at = call.u32(1);
    call.write_u32(at, 0)?;
    call.write_u32(at + 4, name.len() as u32)?;
    Ok(SUCCESS)
}

fn fd_prestat_dir_name(call: &Call) -> Answer {
    let fd = call.u32(0);
    let Some((_, name)) = call.wasi.preopened.iter().find(|(open, _)| *open == fd) else {
        return Ok(EBADF);
    };
    let length = (call.u32(2) as usize).min(name.len());
    call.view.write(call.u32(1), &name.as_bytes()[..length])?;
    Ok(SUCCESS)
}

fn path_open(call: &Call) -> Answer {
    let path = errno!(call.path(call.u32(0), call.u32(2), call.u32(3))?);
    let (flags, rights, fdflags) = (call.u32(4), call.u64(5), call.u32(7));
    let is_directory = path.is_dir();
    if flags & DIRECTORY != 0 || (is_directory && flags & (CREATE | TRUNCATE) == 0) {
        if !is_directory {
            return Ok(match path.exists() {
                true => ENOTDIR,
                false => ENOENT,
            });
        }
        let fd = call.wasi.add(Open::Directory(path));
        call.write_u32(call.u32(8), fd)?;
        return Ok(SUCCESS);
    }
    let write = rights & RIGHT_WRITE != 0 || flags & (CREATE | TRUNCATE) != 0;
    let append = fdflags & APPEND != 0;
    let mut options = OpenOptions::new();
    options
        .read(rights & RIGHT_READ != 0 || !write)
        .write(write && !append)
        .append(append)
        .truncate(flags & TRUNCATE != 0);
    match (flags & CREATE != 0, flags & EXCLUSIVE != 0) {
        (true, true) => options.create_new(true),
        (true, false) => options.create(true),
        (false, _) => &mut options,
    };
    let file = errno!(io(options.open(&path)));
    let fd = call.wasi.add(Open::File { file, append });
    call.write_u32(call.u32(8), fd)?;
    Ok(SUCCESS)
}

fn fd_readdir(call: &Call) -> Answer {
    let path = match call.wasi.open().get(&call.u32(0)) {
        Some(Open::Directory(path)) => path.clone(),
        Some(Open::File { .. }) => return Ok(ENOTDIR),
        None => return Ok(EBADF),
    };
    let (buffer, length, cookie, used) = (call.u32(1), call.u32(2), call.u64(3), call.u32(4));
    let mut entries = vec![(".".to_owned(), FILETYPE_DIRECTORY, 0u64)];
    entries.push(("..".to_owned(), FILETYPE_DIRECTORY, 0));
    let mut listed: Vec<(String, u8, u64)> = errno!(io(fs::read_dir(&path)))
        .filter_map(Result::ok)
        .map(|entry| {
            let kind = match entry.file_type() {
                Ok(kind) if kind.is_dir() => FILETYPE_DIRECTORY,
                Ok(kind) if kind.is_symlink() => FILETYPE_SYMBOLIC_LINK,
                Ok(kind) if kind.is_file() => FILETYPE_REGULAR_FILE,
                _ => 0,
            };
            let inode = entry.metadata().map_or(0, |metadata| identity(&metadata).1);
            (entry.file_name().to_string_lossy().into_owned(), kind, inode)
        })
        .collect();
    listed.sort();
    entries.extend(listed);
    let mut bytes = Vec::new();
    for (index, (name, kind, inode)) in entries.iter().enumerate().skip(cookie as usize) {
        let mut entry = [0u8; DIRENT];
        entry[0..8].copy_from_slice(&(index as u64 + 1).to_le_bytes());
        entry[8..16].copy_from_slice(&inode.to_le_bytes());
        entry[16..20].copy_from_slice(&(name.len() as u32).to_le_bytes());
        entry[20] = *kind;
        bytes.extend_from_slice(&entry);
        bytes.extend_from_slice(name.as_bytes());
        if bytes.len() >= length as usize {
            break;
        }
    }
    bytes.truncate(length as usize);
    call.view.write(buffer, &bytes)?;
    call.write_u32(used, bytes.len() as u32)?;
    Ok(SUCCESS)
}

fn path_create_directory(call: &Call) -> Answer {
    let path = errno!(call.path(call.u32(0), call.u32(1), call.u32(2))?);
    Ok(errno!(io(fs::create_dir(path)).map(|()| SUCCESS)))
}

fn path_remove_directory(call: &Call) -> Answer {
    let path = errno!(call.path(call.u32(0), call.u32(1), call.u32(2))?);
    Ok(errno!(io(fs::remove_dir(path)).map(|()| SUCCESS)))
}

fn path_unlink_file(call: &Call) -> Answer {
    let path = errno!(call.path(call.u32(0), call.u32(1), call.u32(2))?);
    if path.is_dir() {
        return Ok(EISDIR);
    }
    Ok(errno!(io(fs::remove_file(path)).map(|()| SUCCESS)))
}

fn path_rename(call: &Call) -> Answer {
    let from = errno!(call.path(call.u32(0), call.u32(1), call.u32(2))?);
    let to = errno!(call.path(call.u32(3), call.u32(4), call.u32(5))?);
    Ok(errno!(io(fs::rename(from, to)).map(|()| SUCCESS)))
}

fn counted(call: &Call, items: &[String]) -> Answer {
    call.write_u32(call.u32(0), items.len() as u32)?;
    let size: usize = items.iter().map(|item| item.len() + 1).sum();
    call.write_u32(call.u32(1), size as u32)?;
    Ok(SUCCESS)
}

fn listed(call: &Call, items: &[String]) -> Answer {
    let (mut pointers, mut at) = (call.u32(0), call.u32(1));
    for item in items {
        call.write_u32(pointers, at)?;
        call.view.write(at, item.as_bytes())?;
        call.view.write(at + item.len() as u32, &[0])?;
        pointers += 4;
        at += item.len() as u32 + 1;
    }
    Ok(SUCCESS)
}

fn args_sizes_get(call: &Call) -> Answer {
    counted(call, &call.wasi.arguments)
}

fn args_get(call: &Call) -> Answer {
    listed(call, &call.wasi.arguments)
}

fn environ_sizes_get(call: &Call) -> Answer {
    counted(call, &call.wasi.environment)
}

fn environ_get(call: &Call) -> Answer {
    listed(call, &call.wasi.environment)
}

/// The monotonic clock in nanoseconds, from when the process first asked.
fn monotonic() -> u64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

fn now(clock: u32) -> u64 {
    match clock {
        REALTIME => nanoseconds(Ok(SystemTime::now())),
        _ => monotonic(),
    }
}

fn clock_time_get(call: &Call) -> Answer {
    let clock = call.u32(0);
    if clock > 3 {
        return Ok(EINVAL);
    }
    call.write_u64(call.u32(2), now(clock))?;
    Ok(SUCCESS)
}

fn clock_res_get(call: &Call) -> Answer {
    call.write_u64(call.u32(1), 1)?;
    Ok(SUCCESS)
}

fn random_get(call: &Call) -> Answer {
    let mut bytes = vec![0u8; call.u32(1) as usize];
    getrandom::fill(&mut bytes).map_err(|error| format!("no randomness for the app: {error}"))?;
    call.view.write(call.u32(0), &bytes)?;
    Ok(SUCCESS)
}

fn sched_yield(_: &Call) -> Answer {
    std::thread::yield_now();
    Ok(SUCCESS)
}

/// Waits on clocks only: sleeps until the soonest subscription is due, then reports every one as
/// having fired.
fn poll_oneoff(call: &Call) -> Answer {
    let (subscriptions, events, count, written) =
        (call.u32(0), call.u32(1), call.u32(2), call.u32(3));
    let view = &call.view;
    let mut soonest: Option<u64> = None;
    for index in 0..count {
        let at = subscriptions.wrapping_add(index.wrapping_mul(SUBSCRIPTION));
        if view.read(at + 8, 1)?[0] != CLOCK {
            continue;
        }
        let clock = call.read_u32(at + 16)?;
        let timeout = call.read_u64(at + 24)?;
        let flags = u16::from_le_bytes([view.read(at + 40, 1)?[0], view.read(at + 41, 1)?[0]]);
        let wait = match flags & ABSOLUTE {
            0 => timeout,
            _ => timeout.saturating_sub(now(clock)),
        };
        soonest = Some(soonest.map_or(wait, |soonest| soonest.min(wait)));
    }
    if let Some(wait) = soonest {
        std::thread::sleep(Duration::from_nanos(wait));
    }
    for index in 0..count {
        let from = subscriptions.wrapping_add(index.wrapping_mul(SUBSCRIPTION));
        let at = events.wrapping_add(index.wrapping_mul(EVENT));
        let mut event = [0u8; EVENT as usize];
        event[..8].copy_from_slice(&call.read_u64(from)?.to_le_bytes());
        event[10] = view.read(from + 8, 1)?[0];
        view.write(at, &event)?;
    }
    call.write_u32(written, count)?;
    Ok(SUCCESS)
}

/// Why a guest that called `proc_exit` stopped, which a test run reads its exit code from.
pub(crate) const EXITED: &str = "the guest exited with status ";

fn proc_exit(call: &Call) -> Answer {
    Err(format!("{EXITED}{}", call.u32(0) as i32))
}

fn answer(
    scope: &mut v8::PinScope,
    arguments: &v8::FunctionCallbackArguments,
    returned: &mut v8::ReturnValue<v8::Value>,
    function: fn(&Call) -> Answer,
) {
    // unsafe(ffi): the function's data is its State (state_of).
    let state = unsafe { state_of(arguments.data()) };
    let words = (0..arguments.length())
        .map(|index| {
            let value = arguments.get(index);
            match v8::Local::<v8::BigInt>::try_from(value) {
                Ok(big) => big.u64_value().0,
                Err(_) => value.int32_value(scope).unwrap_or(0) as u32 as u64,
            }
        })
        .collect();
    let wasi = Arc::clone(&state.wasi);
    let call = Call {
        arguments: words,
        view: state.view_in(scope),
        wasi: &wasi,
    };
    match function(&call) {
        Ok(errno) => returned.set_int32(errno),
        Err(stopped) => {
            throw(scope, &stopped);
            state.fatal = Some(stopped);
        }
    }
}

fn unsupported(
    _: &mut v8::PinScope,
    _: v8::FunctionCallbackArguments,
    mut returned: v8::ReturnValue<v8::Value>,
) {
    returned.set_int32(ENOSYS);
}

macro_rules! define_calls {
    ($scope:ident, $defined:ident, $data:ident, $name:expr, $($call:ident)*) => {
        match $name {
            $(stringify!($call) => {
                fn function(
                    scope: &mut v8::PinScope,
                    arguments: v8::FunctionCallbackArguments,
                    mut returned: v8::ReturnValue<v8::Value>,
                ) {
                    answer(scope, &arguments, &mut returned, $call);
                }
                crate::engine::function($scope, $defined, MODULE, $name, $data, function)
            })*
            _ => crate::engine::function($scope, $defined, MODULE, $name, $data, unsupported),
        }
    };
}

/// Every WASI call `imports` names.
pub(crate) fn define<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    defined: &mut Defined<'s>,
    data: v8::Local<'s, v8::Value>,
    imports: &[crate::engine::Import],
) -> Result<(), String> {
    for import in imports.iter().filter(|import| import.module == MODULE) {
        let name = import.name.as_str();
        define_calls!(
            scope, defined, data, name,
            args_get args_sizes_get clock_res_get clock_time_get environ_get environ_sizes_get
            fd_close fd_datasync fd_fdstat_get fd_fdstat_set_flags fd_filestat_get fd_filestat_set_size
            fd_pread fd_prestat_dir_name fd_prestat_get fd_pwrite fd_read fd_readdir fd_seek
            fd_sync fd_tell fd_write path_create_directory path_filestat_get path_open
            path_remove_directory path_rename path_unlink_file poll_oneoff proc_exit random_get
            sched_yield
        )?;
    }
    Ok(())
}
