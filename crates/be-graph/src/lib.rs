use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
};

use be_commit::CommitId;
use be_store::Hash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod access;

pub use access::Access;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum BlockParent {
    #[default]
    Detached,
    Root,
    Block(Uuid),
}

impl BlockParent {
    pub const fn block(self) -> Option<Uuid> {
        match self {
            Self::Block(id) => Some(id),
            Self::Detached | Self::Root => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlockNode {
    pub content_type: Uuid,
    pub author: Uuid,
    pub parent: BlockParent,
    pub head: Option<CommitId>,
    pub references: Vec<Uuid>,
    pub grants: BTreeMap<Uuid, Access>,
    pub metadata: Vec<u8>,
    pub version: u64,
}

impl BlockNode {
    pub fn new(content_type: Uuid, author: Uuid, parent: BlockParent) -> Self {
        Self {
            content_type,
            author,
            parent,
            head: None,
            references: Vec::new(),
            grants: BTreeMap::new(),
            metadata: Vec::new(),
            version: 0,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum GraphError {
    NotFound(Uuid),
    AlreadyExists(Uuid),
    ParentCycle,
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(formatter, "block {id} is not in the graph"),
            Self::AlreadyExists(id) => write!(formatter, "block {id} already exists"),
            Self::ParentCycle => formatter.write_str("the requested parent would form a cycle"),
        }
    }
}

impl Error for GraphError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockGraph {
    blocks: BTreeMap<Uuid, BlockNode>,
    backrefs: BTreeMap<Uuid, BTreeSet<Uuid>>,
    children: HashMap<BlockParent, BTreeSet<Uuid>>,
    entries: HashMap<Uuid, BTreeSet<Uuid>>,
}

fn direct(node: &BlockNode, account: Uuid) -> Access {
    match node.author == account {
        true => Access::Edit,
        false => node.grants.get(&account).copied().unwrap_or_default(),
    }
}

fn unindex<K: Eq + std::hash::Hash>(index: &mut HashMap<K, BTreeSet<Uuid>>, key: K, id: Uuid) {
    if let Some(set) = index.get_mut(&key) {
        set.remove(&id);
        if set.is_empty() {
            index.remove(&key);
        }
    }
}

impl BlockGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn get(&self, id: Uuid) -> Option<&BlockNode> {
        self.blocks.get(&id)
    }

    pub fn contains(&self, id: Uuid) -> bool {
        self.blocks.contains_key(&id)
    }

    pub fn ids(&self) -> Vec<Uuid> {
        self.blocks.keys().copied().collect()
    }

    pub fn insert(&mut self, id: Uuid, node: BlockNode) -> Result<(), GraphError> {
        if self.blocks.contains_key(&id) {
            return Err(GraphError::AlreadyExists(id));
        }
        if let BlockParent::Block(parent) = node.parent
            && !self.blocks.contains_key(&parent)
        {
            return Err(GraphError::NotFound(parent));
        }
        for reference in &node.references {
            self.backrefs.entry(*reference).or_default().insert(id);
        }
        self.children.entry(node.parent).or_default().insert(id);
        self.entries.entry(node.author).or_default().insert(id);
        for account in node.grants.keys() {
            self.entries.entry(*account).or_default().insert(id);
        }
        self.blocks.insert(id, node);
        Ok(())
    }

    pub fn touch(&mut self, id: Uuid) -> Result<u64, GraphError> {
        let node = self.node_mut(id)?;
        node.version += 1;
        Ok(node.version)
    }

    pub fn set_head(&mut self, id: Uuid, head: CommitId) -> Result<(), GraphError> {
        self.node_mut(id)?.head = Some(head);
        Ok(())
    }

    pub fn set_metadata(&mut self, id: Uuid, metadata: Vec<u8>) -> Result<(), GraphError> {
        self.node_mut(id)?.metadata = metadata;
        Ok(())
    }

    pub fn head(&self, id: Uuid) -> Option<CommitId> {
        self.blocks.get(&id).and_then(|node| node.head)
    }

    pub fn set_parent(&mut self, id: Uuid, parent: BlockParent) -> Result<(), GraphError> {
        if !self.blocks.contains_key(&id) {
            return Err(GraphError::NotFound(id));
        }
        if let BlockParent::Block(target) = parent {
            if !self.blocks.contains_key(&target) {
                return Err(GraphError::NotFound(target));
            }
            if target == id || self.ancestors(target).any(|ancestor| ancestor == id) {
                return Err(GraphError::ParentCycle);
            }
        }
        let previous = std::mem::replace(&mut self.node_mut(id)?.parent, parent);
        unindex(&mut self.children, previous, id);
        self.children.entry(parent).or_default().insert(id);
        Ok(())
    }

    pub fn set_references(&mut self, id: Uuid, references: &[Uuid]) -> Result<(), GraphError> {
        let mut ordered = Vec::with_capacity(references.len());
        for reference in references {
            if !ordered.contains(reference) {
                ordered.push(*reference);
            }
        }
        let previous = std::mem::replace(&mut self.node_mut(id)?.references, ordered.clone());
        for reference in previous {
            if !ordered.contains(&reference)
                && let Some(backrefs) = self.backrefs.get_mut(&reference)
            {
                backrefs.remove(&id);
                if backrefs.is_empty() {
                    self.backrefs.remove(&reference);
                }
            }
        }
        for reference in ordered {
            self.backrefs.entry(reference).or_default().insert(id);
        }
        Ok(())
    }

    pub fn references(&self, id: Uuid) -> Vec<Uuid> {
        self.blocks
            .get(&id)
            .map(|node| node.references.clone())
            .unwrap_or_default()
    }

    pub fn backrefs(&self, id: Uuid) -> Vec<Uuid> {
        self.backrefs
            .get(&id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn children(&self, parent: BlockParent) -> Vec<Uuid> {
        self.children
            .get(&parent)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    fn child_ids(&self, parent: Uuid) -> impl Iterator<Item = Uuid> + '_ {
        self.children
            .get(&BlockParent::Block(parent))
            .into_iter()
            .flatten()
            .copied()
    }

    pub fn ancestors(&self, id: Uuid) -> impl Iterator<Item = Uuid> + '_ {
        std::iter::successors(Some(id), |current| {
            self.blocks
                .get(current)
                .and_then(|node| node.parent.block())
        })
        .skip(1)
    }

    pub fn is_live(&self, id: Uuid) -> bool {
        let mut current = id;
        while let Some(node) = self.blocks.get(&current) {
            match node.parent {
                BlockParent::Root => return true,
                BlockParent::Detached => return false,
                BlockParent::Block(parent) => current = parent,
            }
        }
        false
    }

    pub fn subtree(&self, root: Uuid) -> Vec<Uuid> {
        if !self.blocks.contains_key(&root) {
            return Vec::new();
        }
        let mut collected = vec![root];
        let mut next = 0;
        while let Some(id) = collected.get(next).copied() {
            collected.extend(self.child_ids(id));
            next += 1;
        }
        collected
    }

    pub fn detached(&self) -> Vec<Uuid> {
        let mut detached: Vec<Uuid> = self
            .children(BlockParent::Detached)
            .into_iter()
            .flat_map(|root| self.subtree(root))
            .collect();
        detached.sort_unstable();
        detached
    }

    pub fn remove(&mut self, id: Uuid) -> Option<BlockNode> {
        let node = self.blocks.remove(&id)?;
        for reference in &node.references {
            if let Some(backrefs) = self.backrefs.get_mut(reference) {
                backrefs.remove(&id);
                if backrefs.is_empty() {
                    self.backrefs.remove(reference);
                }
            }
        }
        for referrer in self.backrefs.remove(&id).unwrap_or_default() {
            if let Some(other) = self.blocks.get_mut(&referrer) {
                other.references.retain(|reference| *reference != id);
            }
        }
        unindex(&mut self.children, node.parent, id);
        for child in self
            .children
            .remove(&BlockParent::Block(id))
            .unwrap_or_default()
        {
            if let Some(other) = self.blocks.get_mut(&child) {
                other.parent = BlockParent::Detached;
            }
            self.children
                .entry(BlockParent::Detached)
                .or_default()
                .insert(child);
        }
        unindex(&mut self.entries, node.author, id);
        for account in node.grants.keys() {
            if *account != node.author {
                unindex(&mut self.entries, *account, id);
            }
        }
        Some(node)
    }

    pub fn grant(&mut self, id: Uuid, account: Uuid, access: Access) -> Result<(), GraphError> {
        let node = self.node_mut(id)?;
        let author = node.author;
        if access == Access::None {
            node.grants.remove(&account);
            if account != author {
                unindex(&mut self.entries, account, id);
            }
        } else {
            node.grants.insert(account, access);
            self.entries.entry(account).or_default().insert(id);
        }
        Ok(())
    }

    pub fn access(&self, id: Uuid, account: Uuid) -> Access {
        let mut current = Some(id);
        let mut best = Access::None;
        while let Some(block) = current {
            let Some(node) = self.blocks.get(&block) else {
                break;
            };
            best = best.max(node.grants.get(&account).copied().unwrap_or_default());
            if best == Access::Edit {
                return best;
            }
            current = node.parent.block();
        }
        best
    }

    pub fn entry_points(&self, account: Uuid) -> Vec<Uuid> {
        self.entries
            .get(&account)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn inherited(&self, id: Uuid, account: Uuid) -> Access {
        let Some(node) = self.blocks.get(&id) else {
            return Access::None;
        };
        let mut best = direct(node, account);
        for ancestor in self.ancestors(id) {
            if best == Access::Edit {
                break;
            }
            let Some(above) = self.blocks.get(&ancestor) else {
                break;
            };
            let inherited = direct(above, account);
            if inherited.can_view() {
                best = best.max(inherited);
            }
        }
        best
    }

    pub fn effective_access(&self, id: Uuid, account: Uuid) -> Access {
        let level = self.inherited(id, account);
        if level.can_know_exists() {
            return level;
        }
        if self.referenced_by_viewer(id, account) || self.knows_below(id, account) {
            return Access::KnowExists;
        }
        Access::None
    }

    fn referenced_by_viewer(&self, id: Uuid, account: Uuid) -> bool {
        let Some(node) = self.blocks.get(&id) else {
            return false;
        };
        self.backrefs.get(&id).is_some_and(|referrers| {
            referrers.iter().any(|referrer| {
                node.parent != BlockParent::Block(*referrer)
                    && self.inherited(*referrer, account).can_view()
            })
        })
    }

    fn knows_below(&self, id: Uuid, account: Uuid) -> bool {
        let entered = self.entries.get(&account).is_some_and(|entries| {
            entries.iter().any(|entry| {
                *entry != id
                    && self
                        .blocks
                        .get(entry)
                        .is_some_and(|node| direct(node, account).can_know_exists())
                    && self.ancestors(*entry).any(|ancestor| ancestor == id)
            })
        });
        entered
            || self
                .subtree(id)
                .into_iter()
                .skip(1)
                .any(|below| self.referenced_by_viewer(below, account))
    }

    pub fn visible(&self, account: Uuid) -> BTreeMap<Uuid, Access> {
        let mut levels: BTreeMap<Uuid, Access> = BTreeMap::new();
        fn raise(levels: &mut BTreeMap<Uuid, Access>, id: Uuid, level: Access) {
            let current = levels.entry(id).or_default();
            *current = (*current).max(level);
        }
        let mut roots = Vec::new();
        for entry in self.entry_points(account) {
            let Some(node) = self.blocks.get(&entry) else {
                continue;
            };
            let level = direct(node, account);
            if !level.can_view() {
                if level.can_know_exists() {
                    raise(&mut levels, entry, level);
                }
                continue;
            }
            let covered = self.ancestors(entry).any(|ancestor| {
                self.blocks
                    .get(&ancestor)
                    .is_some_and(|above| direct(above, account).can_view())
            });
            if !covered {
                roots.push((entry, level));
            }
        }
        let mut stack = roots;
        while let Some((id, level)) = stack.pop() {
            raise(&mut levels, id, level);
            for child in self.child_ids(id) {
                let own = self
                    .blocks
                    .get(&child)
                    .map_or(Access::None, |node| direct(node, account));
                stack.push((child, level.max(own)));
            }
        }
        let referenced: Vec<Uuid> = levels
            .iter()
            .filter(|(_, level)| level.can_view())
            .flat_map(|(id, _)| {
                self.references(*id).into_iter().filter(move |reference| {
                    self.blocks
                        .get(reference)
                        .is_some_and(|target| target.parent != BlockParent::Block(*id))
                })
            })
            .collect();
        for id in referenced {
            raise(&mut levels, id, Access::KnowExists);
        }
        let known: Vec<Uuid> = levels.keys().copied().collect();
        let mut climbed = BTreeSet::new();
        for id in known {
            for ancestor in self.ancestors(id) {
                if !self.blocks.contains_key(&ancestor) || !climbed.insert(ancestor) {
                    break;
                }
                raise(&mut levels, ancestor, Access::KnowExists);
            }
        }
        levels
    }

    pub fn viewers(&self, id: Uuid) -> BTreeSet<Uuid> {
        let mut viewers = BTreeSet::new();
        for block in std::iter::once(id).chain(self.ancestors(id)) {
            let Some(node) = self.blocks.get(&block) else {
                break;
            };
            viewers.insert(node.author);
            viewers.extend(
                node.grants
                    .iter()
                    .filter(|(_, level)| level.can_view())
                    .map(|(account, _)| *account),
            );
        }
        viewers
    }

    pub fn audience(&self, id: Uuid) -> BTreeSet<Uuid> {
        let mut audience = BTreeSet::new();
        if !self.blocks.contains_key(&id) {
            return audience;
        }
        if let Some(parent) = self.blocks.get(&id).and_then(|node| node.parent.block()) {
            audience.extend(self.viewers(parent));
        }
        for below in self.subtree(id) {
            let Some(node) = self.blocks.get(&below) else {
                continue;
            };
            audience.insert(node.author);
            audience.extend(
                node.grants
                    .iter()
                    .filter(|(_, level)| level.can_know_exists())
                    .map(|(account, _)| *account),
            );
            for referrer in self.backrefs.get(&below).into_iter().flatten() {
                if node.parent != BlockParent::Block(*referrer) {
                    audience.extend(self.viewers(*referrer));
                }
            }
        }
        audience
    }

    fn node_mut(&mut self, id: Uuid) -> Result<&mut BlockNode, GraphError> {
        self.blocks.get_mut(&id).ok_or(GraphError::NotFound(id))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObjectRefs {
    counts: BTreeMap<Hash, u32>,
}

impl ObjectRefs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn count(&self, hash: Hash) -> u32 {
        self.counts.get(&hash).copied().unwrap_or(0)
    }

    pub fn len(&self) -> usize {
        self.counts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    pub fn retain(&mut self, hashes: &[Hash]) -> Vec<Hash> {
        let mut fresh = Vec::new();
        for hash in hashes {
            let count = self.counts.entry(*hash).or_insert(0);
            if *count == 0 {
                fresh.push(*hash);
            }
            *count += 1;
        }
        fresh
    }

    pub fn release(&mut self, hashes: &[Hash]) -> Vec<Hash> {
        let mut freed = Vec::new();
        for hash in hashes {
            let Some(count) = self.counts.get_mut(hash) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.counts.remove(hash);
                freed.push(*hash);
            }
        }
        freed
    }
}

#[cfg(test)]
mod tests;
