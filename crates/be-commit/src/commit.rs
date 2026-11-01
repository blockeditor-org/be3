use std::fmt;

use be_store::{Hash, Manifest};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CommitId(Hash);

impl CommitId {
    pub const fn from_hash(hash: Hash) -> Self {
        Self(hash)
    }

    pub const fn hash(self) -> Hash {
        self.0
    }

    pub fn short(self) -> String {
        self.0.to_hex().chars().take(8).collect()
    }
}

impl fmt::Debug for CommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "CommitId({})", self.short())
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0.to_hex())
    }
}

#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ChangeId(Uuid);

impl ChangeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn of_commit(id: CommitId) -> Self {
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&id.hash().as_bytes()[..16]);
        Self(Uuid::from_bytes(bytes))
    }

    pub const fn uuid(self) -> Uuid {
        self.0
    }

    pub fn short(self) -> String {
        self.0.simple().to_string().chars().take(8).collect()
    }
}

impl Default for ChangeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ChangeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ChangeId({})", self.short())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommitKind {
    Autosave,
    Bookmark(String),
    Merge,
    Import,
    Snapshot(String),
}

impl CommitKind {
    pub fn is_pinned(&self) -> bool {
        matches!(self, Self::Bookmark(_))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Commit {
    pub parents: Vec<CommitId>,
    pub manifest: Manifest,
    pub author: Uuid,
    pub time: i64,
    pub kind: CommitKind,
    pub references: Vec<Uuid>,
    #[serde(with = "be_store::bytes")]
    pub metadata: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<ChangeId>,
}

impl Commit {
    pub fn new(manifest: Manifest, author: Uuid, time: i64) -> Self {
        Self {
            parents: Vec::new(),
            manifest,
            author,
            time,
            kind: CommitKind::Autosave,
            references: Vec::new(),
            metadata: Vec::new(),
            change: None,
        }
    }

    pub fn change_id(&self, id: CommitId) -> ChangeId {
        self.change.unwrap_or_else(|| ChangeId::of_commit(id))
    }

    pub fn with_change(mut self, change: ChangeId) -> Self {
        self.change = Some(change);
        self
    }

    pub fn with_parent(mut self, parent: Option<CommitId>) -> Self {
        self.parents = parent.into_iter().collect();
        self
    }

    pub fn with_parents(mut self, parents: Vec<CommitId>) -> Self {
        self.parents = parents;
        self
    }

    pub fn with_kind(mut self, kind: CommitKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_metadata(mut self, metadata: Vec<u8>) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn with_references(mut self, references: Vec<Uuid>) -> Self {
        self.references = references;
        self
    }
}
