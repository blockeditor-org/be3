use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
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
        let references = node.references.clone();
        self.blocks.insert(id, node);
        for reference in references {
            self.backrefs.entry(reference).or_default().insert(id);
        }
        Ok(())
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
            let mut ancestor = Some(target);
            while let Some(current) = ancestor {
                if current == id {
                    return Err(GraphError::ParentCycle);
                }
                ancestor = self
                    .blocks
                    .get(&current)
                    .and_then(|node| node.parent.block());
            }
        }
        self.node_mut(id)?.parent = parent;
        Ok(())
    }

    pub fn apply_references(
        &mut self,
        id: Uuid,
        added: &[Uuid],
        removed: &[Uuid],
    ) -> Result<(), GraphError> {
        if !self.blocks.contains_key(&id) {
            return Err(GraphError::NotFound(id));
        }
        for reference in removed {
            self.node_mut(id)?
                .references
                .retain(|held| held != reference);
            if let Some(backrefs) = self.backrefs.get_mut(reference) {
                backrefs.remove(&id);
                if backrefs.is_empty() {
                    self.backrefs.remove(reference);
                }
            }
        }
        for reference in added {
            let node = self.node_mut(id)?;
            if !node.references.contains(reference) {
                node.references.push(*reference);
            }
            self.backrefs.entry(*reference).or_default().insert(id);
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
        self.blocks
            .iter()
            .filter(|(_, node)| node.parent == parent)
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn is_live(&self, id: Uuid) -> bool {
        let mut current = id;
        let mut steps = 0;
        while let Some(node) = self.blocks.get(&current) {
            match node.parent {
                BlockParent::Root => return true,
                BlockParent::Detached => return false,
                BlockParent::Block(parent) => current = parent,
            }
            steps += 1;
            if steps > self.blocks.len() {
                return false;
            }
        }
        false
    }

    pub fn subtree(&self, root: Uuid) -> Vec<Uuid> {
        let mut collected = Vec::new();
        let mut queue = VecDeque::from([root]);
        let mut seen = BTreeSet::new();
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) || !self.blocks.contains_key(&id) {
                continue;
            }
            collected.push(id);
            queue.extend(self.children(BlockParent::Block(id)));
        }
        collected
    }

    pub fn detached(&self) -> Vec<Uuid> {
        self.blocks
            .keys()
            .copied()
            .filter(|id| !self.is_live(*id))
            .collect()
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
        self.backrefs.remove(&id);
        for other in self.blocks.values_mut() {
            other.references.retain(|reference| *reference != id);
            if other.parent == BlockParent::Block(id) {
                other.parent = BlockParent::Detached;
            }
        }
        Some(node)
    }

    pub fn grant(&mut self, id: Uuid, account: Uuid, access: Access) -> Result<(), GraphError> {
        let node = self.node_mut(id)?;
        if access == Access::None {
            node.grants.remove(&account);
        } else {
            node.grants.insert(account, access);
        }
        Ok(())
    }

    pub fn access(&self, id: Uuid, account: Uuid) -> Access {
        let mut current = Some(id);
        let mut best = Access::None;
        let mut steps = 0;
        while let Some(block) = current {
            let Some(node) = self.blocks.get(&block) else {
                break;
            };
            best = best.max(node.grants.get(&account).copied().unwrap_or_default());
            if best == Access::Edit {
                return best;
            }
            current = node.parent.block();
            steps += 1;
            if steps > self.blocks.len() {
                break;
            }
        }
        best
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
