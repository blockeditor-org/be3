use std::collections::{HashMap, HashSet, VecDeque};

use be_block_store::{ObjectStore, Part, StoreError, Vault};
use uuid::Uuid;

use crate::{Commit, CommitId};

/// Walks the commit DAG tolerating pruned history: a missing commit truncates a chain instead
/// of breaking it, so `first_parent_chain`, `ancestry` and `common_ancestor` keep working after
/// retention runs.
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
    pub fn write<'a>(
        &self,
        key: Uuid,
        content_type: Uuid,
        data: impl Into<Part<'a>>,
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

    /// Walks `descendant`'s history only until it reaches `ancestor`.
    pub fn is_ancestor(
        &self,
        ancestor: CommitId,
        descendant: CommitId,
    ) -> Result<bool, StoreError> {
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([descendant]);
        while let Some(id) = queue.pop_front() {
            if id == ancestor {
                return Ok(true);
            }
            if !seen.insert(id) {
                continue;
            }
            queue.extend(self.parents_of(id)?);
        }
        Ok(false)
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

/// Where two heads' histories meet, as far as a `MergeBaseWalk` looked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeBase {
    /// `theirs` is in `ours`'s history: everything it holds, `ours` holds too.
    Behind,
    /// The common ancestor nearest the two heads.
    Base(CommitId),
    /// The two histories share no commit: each was walked to its root.
    Unrelated,
    /// They did not meet within the walk's depth, or before a side reached a commit the store
    /// lacks (retention thinned it away), which says nothing of what lies past it.
    Unreached,
}

/// A walk from two heads back to their common ancestor, a level of parents on each side at a
/// time, that gives up past a depth: a merge wants the nearest base, and history deeper than
/// that is not worth reading for it. The caller fetches `frontier` before each `step`, so a
/// peer reads from the server only the commits the walk reaches. A commit the store lacks ends
/// its chain, but not as a root does: a walk that ends there is `Unreached`, not `Unrelated`.
pub struct MergeBaseWalk {
    theirs: CommitId,
    sides: [Side; 2],
    level: usize,
    depth: usize,
}

struct Side {
    /// Each commit reached, by its distance from the head.
    seen: HashMap<CommitId, usize>,
    /// The commits reached at the last level, whose parents the next step reads.
    frontier: Vec<CommitId>,
    /// Whether it reached a commit the store lacks.
    cut: bool,
}

impl MergeBaseWalk {
    /// A walk that reads at most `depth` levels of parents on each side.
    pub fn new(ours: CommitId, theirs: CommitId, depth: usize) -> Self {
        let side = |head| Side {
            seen: HashMap::from([(head, 0)]),
            frontier: vec![head],
            cut: false,
        };
        Self {
            theirs,
            sides: [side(ours), side(theirs)],
            level: 0,
            depth,
        }
    }

    /// The commits whose parents the next `step` reads, to fetch first.
    pub fn frontier(&self) -> impl Iterator<Item = CommitId> + '_ {
        self.sides
            .iter()
            .flat_map(|side| side.frontier.iter().copied())
    }

    /// How many commits the walk reached on the two sides: those whose parents it read, and
    /// those it would read next.
    pub fn reached(&self) -> usize {
        self.sides.iter().map(|side| side.seen.len()).sum()
    }

    /// Takes one level of parents on each side; answers once the walk is over, before it
    /// reaches commits it would have to fetch.
    pub fn step<S: ObjectStore>(
        &mut self,
        store: &CommitStore<S>,
    ) -> Result<Option<MergeBase>, StoreError> {
        if let Some(over) = self.over() {
            return Ok(Some(over));
        }
        self.level += 1;
        for side in &mut self.sides {
            let mut next = Vec::new();
            for id in std::mem::take(&mut side.frontier) {
                let Some(commit) = store.try_get(id)? else {
                    side.cut = true;
                    continue;
                };
                for parent in commit.parents {
                    if let std::collections::hash_map::Entry::Vacant(e) = side.seen.entry(parent) {
                        e.insert(self.level);
                        next.push(parent);
                    }
                }
            }
            side.frontier = next;
        }
        Ok(self.over())
    }

    fn over(&self) -> Option<MergeBase> {
        let [ours, theirs] = &self.sides;
        if ours.seen.contains_key(&self.theirs) {
            return Some(MergeBase::Behind);
        }
        // Only the commits the last level reached can be new meetings.
        let nearest = ours
            .frontier
            .iter()
            .chain(&theirs.frontier)
            .filter_map(|id| Some((ours.seen.get(id)? + theirs.seen.get(id)?, *id)))
            .min_by_key(|(distance, _)| *distance);
        if let Some((_, base)) = nearest {
            return Some(MergeBase::Base(base));
        }
        if ours.frontier.is_empty() && theirs.frontier.is_empty() {
            return Some(match ours.cut || theirs.cut {
                true => MergeBase::Unreached,
                false => MergeBase::Unrelated,
            });
        }
        (self.level == self.depth).then_some(MergeBase::Unreached)
    }
}
