use std::collections::{HashMap, HashSet, VecDeque};

use be_store::{ObjectStore, StoreError, Vault};
use uuid::Uuid;

use crate::{Commit, CommitId};

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

    pub fn put(&self, key: Uuid, commit: &Commit) -> Result<CommitId, StoreError> {
        Ok(CommitId::from_hash(self.vault.put_value(key, commit)?))
    }

    pub fn get(&self, id: CommitId) -> Result<Commit, StoreError> {
        self.vault.get_value(id.hash())
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
        self.vault.has_value(id.hash())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn write(
        &self,
        key: Uuid,
        content_type: Uuid,
        data: &[u8],
        author: Uuid,
        time: i64,
        parent: Option<CommitId>,
        references: Vec<Uuid>,
    ) -> Result<CommitId, StoreError> {
        let manifest = self.vault.write(key, content_type, data)?;
        let commit = Commit::new(manifest, author, time)
            .with_parent(parent)
            .with_references(references);
        self.put(key, &commit)
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
