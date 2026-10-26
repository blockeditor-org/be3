use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use be_commit::CommitId;
use be_store::Hash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::STATE_DIRECTORY;

const INDEX_FILE: &str = "index.json";
const LOCK_FILE: &str = "lock";
const VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Index {
    pub version: u32,
    pub organization: Uuid,
    pub root: Uuid,
    pub records: BTreeMap<Uuid, Record>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Record {
    pub parent: Option<Uuid>,
    pub name: String,
    pub block_type: Uuid,
    #[serde(default)]
    pub executable: bool,
    #[serde(default, with = "commit", skip_serializing_if = "Option::is_none")]
    pub head: Option<CommitId>,
    #[serde(default, with = "hash", skip_serializing_if = "Option::is_none")]
    pub content: Option<Hash>,
    pub placed: Placed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "as")]
pub enum Placed {
    Directory {
        path: String,
        #[serde(default)]
        inode: u64,
    },
    File {
        path: String,
        stat: Stat,
    },
    Skipped {
        reason: Skip,
        why: String,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Stat {
    pub size: u64,
    pub modified: i64,
    pub inode: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Skip {
    Name,
    Collision,
    Occupied,
    Ignored,
    NoFileForm,
    Inside,
}

impl Record {
    pub fn path(&self) -> Option<&str> {
        match &self.placed {
            Placed::Directory { path, .. } | Placed::File { path, .. } => Some(path),
            Placed::Skipped { .. } => None,
        }
    }

    pub fn is_directory(&self) -> bool {
        matches!(self.placed, Placed::Directory { .. })
    }
}

impl Index {
    pub fn new(organization: Uuid, root: Uuid) -> Self {
        Self {
            version: VERSION,
            organization,
            root,
            records: BTreeMap::new(),
        }
    }

    pub fn load(directory: &Path) -> Result<Option<Self>, String> {
        let path = directory.join(STATE_DIRECTORY).join(INDEX_FILE);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("could not read {}: {error}", path.display())),
        };
        let index: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{} is damaged: {error}", path.display()))?;
        if index.version != VERSION {
            return Err(format!(
                "{} is version {}, and this be reads version {VERSION}",
                path.display(),
                index.version
            ));
        }
        Ok(Some(index))
    }

    pub fn save(&self, directory: &Path) -> Result<(), String> {
        let state = directory.join(STATE_DIRECTORY);
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        crate::disk::write_atomically(&state, &state.join(INDEX_FILE), &bytes)
    }
}

pub struct Lock {
    _file: File,
}

impl Lock {
    pub fn take(directory: &Path) -> Result<Self, String> {
        let state = directory.join(STATE_DIRECTORY);
        std::fs::create_dir_all(&state)
            .map_err(|error| format!("could not make {}: {error}", state.display()))?;
        let path = state.join(LOCK_FILE);
        let file = File::create(&path)
            .map_err(|error| format!("could not open {}: {error}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => Err(format!(
                "another be sync is already syncing {}",
                directory.display()
            )),
            Err(std::fs::TryLockError::Error(error)) => {
                Err(format!("could not lock {}: {error}", path.display()))
            }
        }
    }
}

mod commit {
    use be_commit::CommitId;
    use be_store::Hash;
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::ref_option)]
    pub fn serialize<S: Serializer>(
        value: &Option<CommitId>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(id) => serializer.serialize_str(&id.hash().to_hex()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<CommitId>, D::Error> {
        let text: Option<String> = Option::deserialize(deserializer)?;
        text.map(|text| {
            Hash::from_hex(&text)
                .map(CommitId::from_hash)
                .ok_or_else(|| serde::de::Error::custom("a commit id is 64 hex digits"))
        })
        .transpose()
    }
}

mod hash {
    use be_store::Hash;
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::ref_option)]
    pub fn serialize<S: Serializer>(
        value: &Option<Hash>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(hash) => serializer.serialize_str(&hash.to_hex()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Hash>, D::Error> {
        let text: Option<String> = Option::deserialize(deserializer)?;
        text.map(|text| {
            Hash::from_hex(&text).ok_or_else(|| serde::de::Error::custom("a hash is 64 hex digits"))
        })
        .transpose()
    }
}
