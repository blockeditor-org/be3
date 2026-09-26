use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

use be_store::{Hash, Manifest, ObjectStore, StoreError, Vault};
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
        }
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

    pub fn with_references(mut self, mut references: Vec<Uuid>) -> Self {
        references.sort_unstable();
        references.dedup();
        self.references = references;
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReferenceDelta {
    pub added: usize,
    pub removed: usize,
}

pub fn reference_delta(before: &[Uuid], after: &[Uuid]) -> (Vec<Uuid>, Vec<Uuid>) {
    let before: HashSet<_> = before.iter().copied().collect();
    let after_set: HashSet<_> = after.iter().copied().collect();
    let mut added: Vec<_> = after_set.difference(&before).copied().collect();
    let mut removed: Vec<_> = before.difference(&after_set).copied().collect();
    added.sort_unstable();
    removed.sort_unstable();
    (added, removed)
}

pub struct CommitStore<S> {
    vault: Vault<S>,
}

impl<S: ObjectStore> CommitStore<S> {
    pub fn new(vault: Vault<S>) -> Self {
        Self { vault }
    }

    pub fn vault(&self) -> &Vault<S> {
        &self.vault
    }

    pub fn put(&self, commit: &Commit) -> Result<CommitId, StoreError> {
        Ok(CommitId(self.vault.put_value(commit)?))
    }

    pub fn get(&self, id: CommitId) -> Result<Commit, StoreError> {
        self.vault.get_value(id.0)
    }

    pub fn try_get(&self, id: CommitId) -> Result<Option<Commit>, StoreError> {
        if self.has(id)? {
            self.get(id).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn parents_of(&self, id: CommitId) -> Result<Vec<CommitId>, StoreError> {
        Ok(self
            .try_get(id)?
            .map(|commit| commit.parents)
            .unwrap_or_default())
    }

    pub fn has(&self, id: CommitId) -> Result<bool, StoreError> {
        self.vault.has_value(id.0)
    }

    pub fn write(
        &self,
        content_type: Uuid,
        data: &[u8],
        author: Uuid,
        time: i64,
        parent: Option<CommitId>,
        references: Vec<Uuid>,
    ) -> Result<CommitId, StoreError> {
        let manifest = self.vault.write(content_type, data)?;
        let commit = Commit::new(manifest, author, time)
            .with_parent(parent)
            .with_references(references);
        self.put(&commit)
    }

    pub fn read(&self, id: CommitId) -> Result<Vec<u8>, StoreError> {
        self.vault.read(&self.get(id)?.manifest)
    }

    pub fn read_range(
        &self,
        id: CommitId,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, StoreError> {
        self.vault
            .read_range(&self.get(id)?.manifest, offset, length)
    }

    pub fn first_parent_chain(&self, head: CommitId) -> Result<Vec<CommitId>, StoreError> {
        let mut chain = vec![head];
        let mut current = head;
        while let Some(parent) = self.parents_of(current)?.first().copied() {
            chain.push(parent);
            current = parent;
        }
        Ok(chain)
    }

    pub fn ancestry(&self, head: CommitId) -> Result<HashSet<CommitId>, StoreError> {
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([head]);
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) {
                continue;
            }
            queue.extend(self.parents_of(id)?);
        }
        Ok(seen)
    }

    pub fn is_ancestor(
        &self,
        ancestor: CommitId,
        descendant: CommitId,
    ) -> Result<bool, StoreError> {
        Ok(self.ancestry(descendant)?.contains(&ancestor))
    }

    pub fn common_ancestor(
        &self,
        ours: CommitId,
        theirs: CommitId,
    ) -> Result<Option<CommitId>, StoreError> {
        let mut depths = HashMap::new();
        let mut queue = VecDeque::from([(ours, 0usize)]);
        while let Some((id, depth)) = queue.pop_front() {
            if depths.insert(id, depth).is_some() {
                continue;
            }
            for parent in self.parents_of(id)? {
                queue.push_back((parent, depth + 1));
            }
        }
        let mut best: Option<(CommitId, usize)> = None;
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([theirs]);
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) {
                continue;
            }
            if let Some(&depth) = depths.get(&id) {
                if best.is_none_or(|(_, current)| depth < current) {
                    best = Some((id, depth));
                }
                continue;
            }
            queue.extend(self.parents_of(id)?);
        }
        Ok(best.map(|(id, _)| id))
    }
}
