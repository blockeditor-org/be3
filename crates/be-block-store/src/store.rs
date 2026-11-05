use std::{
    collections::HashMap,
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError, RwLock},
    time::Duration,
};

use crate::{StoreError, hash::Hash};

pub trait ObjectStore: Send + Sync {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError>;

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError>;

    fn has(&self, hash: Hash) -> Result<bool, StoreError>;

    fn remove(&self, hash: Hash) -> Result<(), StoreError>;

    /// Makes every object put so far durable at once (one `syncfs` on Linux and Android for a
    /// `FileStore`). An object has to be synced before anyone else is told it exists, or a
    /// crash can lose an object others already name: `Peer::push` syncs before it uploads, and
    /// the server syncs a `PutObjects` before it answers.
    fn sync(&self) -> Result<(), StoreError> {
        Ok(())
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        Ok(self.get(hash)?.map(|bytes| bytes.len()))
    }
}

#[derive(Clone, Default)]
pub struct MemoryStore {
    objects: Arc<RwLock<HashMap<Hash, Vec<u8>>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn count(&self) -> usize {
        self.objects.read().unwrap().len()
    }

    pub fn total_bytes(&self) -> usize {
        self.objects
            .read()
            .unwrap()
            .values()
            .map(Vec::len)
            .sum::<usize>()
    }

    pub fn hashes(&self) -> Vec<Hash> {
        let mut hashes: Vec<_> = self.objects.read().unwrap().keys().copied().collect();
        hashes.sort_unstable();
        hashes
    }
}

impl ObjectStore for MemoryStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        let hash = Hash::of(bytes);
        self.objects
            .write()
            .unwrap()
            .entry(hash)
            .or_insert_with(|| bytes.to_vec());
        Ok(hash)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.objects.read().unwrap().get(&hash).cloned())
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        Ok(self.objects.read().unwrap().contains_key(&hash))
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        self.objects.write().unwrap().remove(&hash);
        Ok(())
    }
}

/// `put` writes atomically (a temporary file renamed into place) but does not wait for the
/// disk; see `ObjectStore::sync`. A torn object left by a crash fails its hash on reading and
/// is dropped.
#[derive(Clone)]
pub struct FileStore {
    root: PathBuf,
    /// None for a cache (`open_cache`), whose writes are never synced.
    unsynced: Option<Arc<Mutex<Unsynced>>>,
}

#[derive(Default)]
struct Unsynced {
    files: Vec<PathBuf>,
    directories: std::collections::BTreeSet<PathBuf>,
}

impl FileStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        Self::opened(root.into(), Some(Arc::default()))
    }

    /// A store for copies kept elsewhere too, such as be-server's cache in front of a bucket:
    /// `sync` does nothing and nothing is remembered for it, so a crash may lose or tear what
    /// was written (a torn object reads as missing, as in any `FileStore`).
    pub fn open_cache(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        Self::opened(root.into(), None)
    }

    fn opened(root: PathBuf, unsynced: Option<Arc<Mutex<Unsynced>>>) -> Result<Self, StoreError> {
        fs::create_dir_all(&root)?;
        let store = Self { root, unsynced };
        store.remove_abandoned_writes()?;
        Ok(store)
    }

    fn remove_abandoned_writes(&self) -> Result<(), StoreError> {
        for shard in fs::read_dir(&self.root)? {
            let shard = shard?;
            if !shard.file_type()?.is_dir() {
                continue;
            }
            for entry in fs::read_dir(shard.path())? {
                let entry = entry?;
                let abandoned = entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "partial")
                    && entry
                        .metadata()?
                        .modified()?
                        .elapsed()
                        .is_ok_and(|age| age >= ABANDONED_WRITE_AGE);
                if abandoned {
                    remove_if_present(&entry.path())?;
                }
            }
        }
        Ok(())
    }

    /// Every object the store holds, in no order. It reads every shard, so it is for one-time
    /// maintenance, never a request.
    pub fn hashes(&self) -> Result<Vec<Hash>, StoreError> {
        let mut hashes = Vec::new();
        for shard in fs::read_dir(&self.root)? {
            let shard = shard?;
            if !shard.file_type()?.is_dir() {
                continue;
            }
            let prefix = shard.file_name().to_string_lossy().into_owned();
            for entry in fs::read_dir(shard.path())? {
                let name = entry?.file_name().to_string_lossy().into_owned();
                if let Some(hash) = Hash::from_hex(&format!("{prefix}{name}")) {
                    hashes.push(hash);
                }
            }
        }
        Ok(hashes)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, hash: Hash) -> PathBuf {
        let hex = hash.to_hex();
        self.root.join(&hex[..2]).join(&hex[2..])
    }
}

impl ObjectStore for FileStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        let hash = Hash::of(bytes);
        let path = self.path(hash);
        match fs::metadata(&path) {
            Ok(held) if held.len() == bytes.len() as u64 => return Ok(hash),
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let directory = path.parent().ok_or(StoreError::InvalidPath)?;
        let created = !directory.exists();
        if created {
            fs::create_dir_all(directory)?;
        }
        let temporary = directory.join(format!("{}.partial", uuid::Uuid::new_v4()));
        fs::File::create(&temporary)?.write_all(bytes)?;
        fs::rename(&temporary, &path)?;
        let Some(unsynced) = &self.unsynced else {
            return Ok(hash);
        };
        let mut unsynced = unsynced.lock().unwrap_or_else(PoisonError::into_inner);
        if created {
            unsynced.directories.insert(self.root.clone());
        }
        unsynced.directories.insert(directory.to_owned());
        unsynced.files.push(path);
        Ok(hash)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.path(hash);
        match fs::read(&path) {
            Ok(bytes) if Hash::of(&bytes) == hash => Ok(Some(bytes)),
            Ok(_) => {
                remove_if_present(&path)?;
                Ok(None)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        Ok(self.path(hash).exists())
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        remove_if_present(&self.path(hash))
    }

    fn sync(&self) -> Result<(), StoreError> {
        let Some(pending) = &self.unsynced else {
            return Ok(());
        };
        let unsynced = std::mem::take(&mut *pending.lock().unwrap_or_else(PoisonError::into_inner));
        if unsynced.files.is_empty() && unsynced.directories.is_empty() {
            return Ok(());
        }
        let synced = sync_written(&self.root, &unsynced);
        if synced.is_err() {
            let mut held = pending.lock().unwrap_or_else(PoisonError::into_inner);
            held.files.extend(unsynced.files);
            held.directories.extend(unsynced.directories);
        }
        synced
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        match fs::metadata(self.path(hash)) {
            Ok(metadata) => Ok(Some(usize::try_from(metadata.len()).unwrap_or(usize::MAX))),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

const ABANDONED_WRITE_AGE: Duration = Duration::from_secs(60 * 60);

fn remove_if_present(path: &Path) -> Result<(), StoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn sync_written(root: &Path, _: &Unsynced) -> Result<(), StoreError> {
    use std::os::fd::AsRawFd;
    let directory = fs::File::open(root)?;
    // unsafe(ffi)
    match unsafe { libc::syncfs(directory.as_raw_fd()) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error().into()),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn sync_written(_: &Path, unsynced: &Unsynced) -> Result<(), StoreError> {
    for file in &unsynced.files {
        match fs::OpenOptions::new().write(true).open(file) {
            Ok(file) => file.sync_all()?,
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    for directory in &unsynced.directories {
        sync_directory(directory)?;
    }
    Ok(())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn sync_directory(directory: &Path) -> Result<(), StoreError> {
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> Result<(), StoreError> {
    Ok(())
}

impl<S: ObjectStore + ?Sized> ObjectStore for Arc<S> {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        (**self).put(bytes)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        (**self).get(hash)
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        (**self).has(hash)
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        (**self).remove(hash)
    }

    fn sync(&self) -> Result<(), StoreError> {
        (**self).sync()
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        (**self).len(hash)
    }
}
