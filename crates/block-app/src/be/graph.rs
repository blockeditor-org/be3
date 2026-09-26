use std::collections::HashMap;

use be_block::{
    BlockMetadata,
    version_control::{local_id, masked, scope_mask},
};
use be_commit::CommitId;
use be_graph::{Access, BlockParent};
use block_plugin_api::BlockIdRole;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    pub(crate) id: Uuid,
    pub(crate) content_type: Uuid,
    pub(crate) author: Uuid,
    pub(crate) parent: BlockParent,
    pub(crate) access: Access,
    pub(crate) references: Vec<Uuid>,
    pub(crate) metadata: BlockMetadata,
    pub(crate) head: Option<CommitId>,
    pub(crate) version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Scope {
    pub(crate) checkout: Uuid,
    pub(crate) mask: u128,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Query {
    Roots,
    Detached,
    Children(Uuid),
    References(Uuid),
    Backrefs(Uuid),
    Parents(Uuid),
    Block(Uuid),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Graph {
    pub(crate) loaded: bool,
    pub(crate) revision: u64,
    nodes: HashMap<Uuid, Node>,
    pending: HashMap<Uuid, u32>,
}

impl Graph {
    pub(crate) fn load(&mut self, nodes: Vec<Node>) {
        let mut loaded: HashMap<Uuid, Node> =
            nodes.into_iter().map(|node| (node.id, node)).collect();
        for block in self.pending.keys() {
            match self.nodes.remove(block) {
                Some(node) => loaded.insert(*block, node),
                None => loaded.remove(block),
            };
        }
        self.nodes = loaded;
        self.loaded = true;
        self.revision += 1;
    }

    pub(crate) fn put(&mut self, node: Node) {
        let stale = self
            .nodes
            .get(&node.id)
            .is_some_and(|held| held.version > node.version);
        if !stale && !self.pending.contains_key(&node.id) {
            self.insert(node);
        }
    }

    pub(crate) fn change(&mut self, node: Node) {
        *self.pending.entry(node.id).or_default() += 1;
        self.insert(node);
    }

    pub(crate) fn update(&mut self, block: Uuid, change: impl FnOnce(&mut Node)) {
        if let Some(node) = self.nodes.get_mut(&block) {
            *self.pending.entry(block).or_default() += 1;
            change(node);
            self.revision += 1;
        }
    }

    pub(crate) fn settle(&mut self, block: Uuid) -> bool {
        let Some(count) = self.pending.get_mut(&block) else {
            return true;
        };
        *count -= 1;
        if *count == 0 {
            self.pending.remove(&block);
            return true;
        }
        false
    }

    fn insert(&mut self, node: Node) {
        if self.nodes.get(&node.id) != Some(&node) {
            self.nodes.insert(node.id, node);
            self.revision += 1;
        }
    }

    pub(crate) fn set_head(&mut self, block: Uuid, head: Option<CommitId>) {
        if let Some(node) = self.nodes.get_mut(&block)
            && node.head != head
            && head.is_some()
        {
            node.head = head;
            self.revision += 1;
        }
    }

    pub(crate) fn remove(&mut self, block: Uuid) {
        if self.nodes.remove(&block).is_some() {
            self.revision += 1;
        }
    }

    pub(crate) fn nodes(&self) -> Vec<Node> {
        self.nodes.values().cloned().collect()
    }

    pub(crate) fn get(&self, block: Uuid) -> Option<&Node> {
        self.nodes.get(&block)
    }

    pub(crate) fn query(&self, query: Query) -> Vec<Node> {
        let mut found: Vec<Node> = match query {
            Query::Roots => self.with_parent(BlockParent::Root),
            Query::Detached => self.with_parent(BlockParent::Detached),
            Query::Children(parent) => self.with_parent(BlockParent::Block(parent)),
            Query::References(block) => {
                return self.nodes.get(&block).map_or_else(Vec::new, |node| {
                    node.references
                        .iter()
                        .filter_map(|reference| self.nodes.get(reference).cloned())
                        .collect()
                });
            }
            Query::Backrefs(block) => self
                .nodes
                .values()
                .filter(|node| node.references.contains(&block))
                .cloned()
                .collect(),
            Query::Parents(block) => return self.ancestors(block),
            Query::Block(block) => return self.nodes.get(&block).cloned().into_iter().collect(),
        };
        found.sort_by_key(|node| node.id);
        found
    }

    pub(crate) fn scope_of(&self, block: Uuid) -> Option<Scope> {
        let mut seen = 0;
        let mut current = self.nodes.get(&block)?.parent.block();
        while let Some(parent) = current {
            let node = self.nodes.get(&parent)?;
            if node.content_type == <be_block::Checkout as be_block::Root>::CONTENT_TYPE {
                return Some(Scope {
                    checkout: parent,
                    mask: scope_mask(parent),
                });
            }
            seen += 1;
            if seen > self.nodes.len() {
                return None;
            }
            current = node.parent.block();
        }
        None
    }

    pub(crate) fn to_local(&self, scope: Scope, real: Uuid) -> Uuid {
        self.nodes
            .get(&real)
            .map_or(real, |node| local_id(real, &node.metadata, scope.mask))
    }

    pub(crate) fn to_real(&self, scope: Scope, local: Uuid, role: BlockIdRole) -> Uuid {
        if role == BlockIdRole::Created {
            return local;
        }
        let copy = masked(local, scope.mask);
        match self.nodes.get(&copy) {
            Some(node) if node.metadata.local_id == Some(local) => copy,
            _ => local,
        }
    }

    pub(crate) fn subtree(&self, root: Uuid) -> Vec<Uuid> {
        let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for node in self.nodes.values() {
            if let BlockParent::Block(parent) = node.parent {
                children.entry(parent).or_default().push(node.id);
            }
        }
        let mut found = Vec::new();
        if !self.nodes.contains_key(&root) {
            return found;
        }
        let mut stack = vec![root];
        let mut seen = std::collections::HashSet::new();
        while let Some(block) = stack.pop() {
            if !seen.insert(block) {
                continue;
            }
            found.push(block);
            if let Some(below) = children.get(&block) {
                stack.extend(below.iter().copied());
            }
        }
        found
    }

    fn with_parent(&self, parent: BlockParent) -> Vec<Node> {
        self.nodes
            .values()
            .filter(|node| node.parent == parent)
            .cloned()
            .collect()
    }

    fn ancestors(&self, block: Uuid) -> Vec<Node> {
        let mut chain = Vec::new();
        let mut current = self.nodes.get(&block).and_then(|node| node.parent.block());
        while let Some(parent) = current {
            let Some(node) = self.nodes.get(&parent) else {
                break;
            };
            if chain.iter().any(|seen: &Node| seen.id == parent) {
                break;
            }
            chain.push(node.clone());
            current = node.parent.block();
        }
        chain.reverse();
        chain
    }
}
