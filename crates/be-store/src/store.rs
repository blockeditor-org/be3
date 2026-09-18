use std::{
    collections::HashMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::{StoreError, hash::Hash};

pub trait ObjectStore: Send + Sync {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError>;

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError>;

    fn get_range(&self, hash: Hash, offset: usize, length: usize) -> Result<Vec<u8>, StoreError> {
        let bytes = self.get(hash)?.ok_or(StoreError::MissingObject(hash))?;
        let end = offset.saturating_add(length).min(bytes.len());
        Ok(bytes.get(offset..end).unwrap_or_default().to_vec())
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError>;

    fn remove(&self, hash: Hash) -> Result<(), StoreError>;

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

#[derive(Clone)]
pub struct FileStore {
    root: PathBuf,
}

impl FileStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
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
        if path.exists() {
            return Ok(hash);
        }
        let directory = path.parent().ok_or(StoreError::InvalidPath)?;
        fs::create_dir_all(directory)?;
        let temporary = directory.join(format!("{}.partial", uuid::Uuid::new_v4()));
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, &path)?;
        Ok(hash)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        match fs::read(self.path(hash)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        Ok(self.path(hash).exists())
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        match fs::remove_file(self.path(hash)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        match fs::metadata(self.path(hash)) {
            Ok(metadata) => Ok(Some(usize::try_from(metadata.len()).unwrap_or(usize::MAX))),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl<S: ObjectStore + ?Sized> ObjectStore for Arc<S> {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        (**self).put(bytes)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        (**self).get(hash)
    }

    fn get_range(&self, hash: Hash, offset: usize, length: usize) -> Result<Vec<u8>, StoreError> {
        (**self).get_range(hash, offset, length)
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        (**self).has(hash)
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        (**self).remove(hash)
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        (**self).len(hash)
    }
}
