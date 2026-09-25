use std::collections::HashMap;

use be_block::BlockMetadata;
use be_graph::{Access, BlockParent};
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
        if !self.pending.contains_key(&node.id) {
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
