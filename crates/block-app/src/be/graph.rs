use std::collections::{HashMap, HashSet};

use be_block::{
    BlockMetadata, Entries,
    version_control::{local_id, masked, scope_mask},
};
use be_commit::CommitId;
use be_graph::{Access, BlockParent};
pub(crate) use be_protocol::Listing;
use block_plugin_api::BlockIdRole;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    pub(crate) id: Uuid,
    pub(crate) content_type: Uuid,
    pub(crate) author: Option<Uuid>,
    pub(crate) parent: Option<BlockParent>,
    pub(crate) access: Access,
    pub(crate) references: Vec<Uuid>,
    pub(crate) metadata: BlockMetadata,
    pub(crate) head: Option<CommitId>,
    pub(crate) locked: bool,
    pub(crate) held: bool,
    pub(crate) sets_access: bool,
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
    Shared,
    Children(Uuid),
    References(Uuid),
    Backrefs(Uuid),
    Parents(Uuid),
    Block(Uuid),
    Subtree(Uuid),
    OpenSettings(Uuid),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Owner {
    Host,
    Open,
    Version,
    Organization,
    Editor(u64),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum Fetch {
    #[default]
    Wanted,
    Fetching,
    Loaded,
    Failed(String),
}

#[derive(Clone, Debug, Default)]
struct Listed {
    members: HashSet<Uuid>,
    fetch: Fetch,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Graph {
    pub(crate) revision: u64,
    nodes: HashMap<Uuid, Node>,
    wants: HashMap<Owner, Vec<Query>>,
    listings: HashMap<Listing, Listed>,
    unlisted: Vec<Listing>,
    heard: HashMap<Uuid, u64>,
    floor: u64,
    pending: HashMap<Uuid, u32>,
    derived: HashMap<Uuid, Entries>,
    referenced: HashMap<Uuid, Vec<Uuid>>,
    placing: std::collections::HashSet<Uuid>,
}

impl Graph {
    pub(crate) fn want(&mut self, owner: Owner, queries: Vec<Query>) {
        match queries.is_empty() {
            true => self.wants.remove(&owner),
            false => self.wants.insert(owner, queries),
        };
        self.refresh_listings();
    }

    pub(crate) fn wanted(&self, owner: Owner) -> &[Query] {
        self.wants.get(&owner).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn refresh_listings(&mut self) {
        let mut desired = HashSet::new();
        for queries in self.wants.values() {
            for query in queries {
                self.listings_for(*query, &mut desired);
            }
        }
        let before = self.listings.len();
        let unlisted = &mut self.unlisted;
        self.listings.retain(|listing, listed| {
            let kept = desired.contains(listing);
            if !kept && listed.fetch != Fetch::Wanted {
                unlisted.push(*listing);
            }
            kept
        });
        let mut changed = self.listings.len() != before;
        for listing in desired {
            if self.listings.contains_key(&listing) {
                continue;
            }
            let members = self
                .nodes
                .values()
                .filter(|node| member(listing, node, &self.nodes) == Some(true))
                .map(|node| node.id)
                .collect();
            self.listings.insert(
                listing,
                Listed {
                    members,
                    fetch: Fetch::Wanted,
                },
            );
            changed = true;
        }
        if changed {
            self.prune();
            self.revision += 1;
        }
    }

    fn listings_for(&self, query: Query, into: &mut HashSet<Listing>) {
        match query {
            Query::Roots => {
                into.insert(Listing::Children(BlockParent::Root));
            }
            Query::Detached => {
                into.insert(Listing::Deleted);
            }
            Query::Shared => {
                into.insert(Listing::Shared);
            }
            Query::Children(parent) => {
                into.insert(Listing::Children(BlockParent::Block(parent)));
            }
            Query::Backrefs(block) => {
                into.insert(Listing::Backrefs(block));
            }
            Query::Block(block) => {
                into.insert(Listing::Block(block));
            }
            Query::OpenSettings(block) => {
                into.insert(Listing::OpenSettings(block));
            }
            Query::References(block) => {
                into.insert(Listing::Block(block));
                into.insert(Listing::References(block));
            }
            Query::Parents(block) => {
                into.insert(Listing::Block(block));
                let mut current = block;
                for _ in 0..=self.nodes.len() {
                    let Some(BlockParent::Block(parent)) =
                        self.nodes.get(&current).and_then(|node| node.parent)
                    else {
                        break;
                    };
                    if !into.insert(Listing::Block(parent)) {
                        break;
                    }
                    current = parent;
                }
            }
            Query::Subtree(root) => {
                into.insert(Listing::Block(root));
                into.extend(
                    self.subtree(root)
                        .into_iter()
                        .map(|block| Listing::Children(BlockParent::Block(block))),
                );
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn listings_of(&self, query: Query) -> HashSet<Listing> {
        let mut listings = HashSet::new();
        self.listings_for(query, &mut listings);
        listings
    }

    pub(crate) fn take_wanted(&mut self) -> Vec<Listing> {
        let mut found = Vec::new();
        for (listing, listed) in &mut self.listings {
            if listed.fetch == Fetch::Wanted {
                listed.fetch = Fetch::Fetching;
                found.push(*listing);
            }
        }
        found
    }

    pub(crate) fn take_unlisted(&mut self) -> Vec<Listing> {
        std::mem::take(&mut self.unlisted)
    }

    pub(crate) fn listed(&mut self, listing: Listing, nodes: Vec<Node>, revision: u64) {
        let Some(listed) = self.listings.get_mut(&listing) else {
            return;
        };
        listed.fetch = Fetch::Loaded;
        listed.members = nodes.iter().map(|node| node.id).collect();
        listed.members.extend(
            self.nodes
                .values()
                .filter(|node| {
                    self.pending.contains_key(&node.id)
                        || self
                            .heard
                            .get(&node.id)
                            .is_some_and(|heard| *heard > revision)
                })
                .filter(|node| member(listing, node, &self.nodes) == Some(true))
                .map(|node| node.id),
        );
        self.floor = self.floor.max(revision);
        for node in nodes {
            if self.hear(node.id, revision) && !self.pending.contains_key(&node.id) {
                self.insert(node);
            }
        }
        self.prune();
        self.revision += 1;
        self.refresh_listings();
    }

    pub(crate) fn failed(&mut self, listing: Listing, error: String) {
        let Some(listed) = self.listings.get_mut(&listing) else {
            return;
        };
        listed.fetch = Fetch::Failed(error);
        self.revision += 1;
    }

    pub(crate) fn relist(&mut self, listing: Listing) {
        if let Some(listed) = self.listings.get_mut(&listing)
            && matches!(listed.fetch, Fetch::Loaded | Fetch::Failed(_))
        {
            listed.fetch = Fetch::Wanted;
        }
    }

    pub(crate) fn relist_open_settings(&mut self) {
        let open: Vec<Listing> = self
            .listings
            .keys()
            .filter(|listing| matches!(listing, Listing::OpenSettings(_)))
            .copied()
            .collect();
        for listing in open {
            self.relist(listing);
        }
    }

    pub(crate) fn relist_owner(&mut self, owner: Owner) {
        let mut held = HashSet::new();
        for query in self.wanted(owner) {
            self.listings_for(*query, &mut held);
        }
        for listing in held {
            self.relist(listing);
        }
    }

    pub(crate) fn relist_all(&mut self) {
        self.heard.clear();
        self.floor = 0;
        for listed in self.listings.values_mut() {
            listed.fetch = Fetch::Wanted;
        }
    }

    pub(crate) fn ready(&self, query: Query) -> bool {
        let mut needed = HashSet::new();
        self.listings_for(query, &mut needed);
        needed.iter().all(|listing| {
            let present =
                matches!(listing, Listing::Block(block) if self.nodes.contains_key(block));
            present
                || self
                    .listings
                    .get(listing)
                    .is_some_and(|listed| listed.fetch == Fetch::Loaded)
        })
    }

    pub(crate) fn failure(&self, query: Query) -> Option<String> {
        let mut needed = HashSet::new();
        self.listings_for(query, &mut needed);
        needed.iter().find_map(|listing| {
            if matches!(listing, Listing::Block(block) if self.nodes.contains_key(block)) {
                return None;
            }
            match &self.listings.get(listing)?.fetch {
                Fetch::Failed(error) => Some(error.clone()),
                _ => None,
            }
        })
    }

    pub(crate) fn fetching(&self) -> bool {
        self.listings
            .values()
            .any(|listed| matches!(listed.fetch, Fetch::Wanted | Fetch::Fetching))
    }

    pub(crate) fn covers(&self, block: Uuid) -> bool {
        self.pending.contains_key(&block)
            || self
                .listings
                .values()
                .any(|listed| listed.members.contains(&block))
    }

    fn prune(&mut self) {
        let kept: Vec<Uuid> = self
            .nodes
            .keys()
            .copied()
            .filter(|block| !self.covers(*block))
            .collect();
        for block in kept {
            self.nodes.remove(&block);
            self.revision += 1;
        }
    }

    pub(crate) fn put(&mut self, node: Node, revision: u64) {
        if self.hear(node.id, revision) && !self.pending.contains_key(&node.id) {
            self.insert(node);
        }
    }

    pub(crate) fn remove(&mut self, block: Uuid, revision: u64) {
        if self.hear(block, revision) {
            self.forget(block);
        }
    }

    fn hear(&mut self, block: Uuid, revision: u64) -> bool {
        let newest = self.heard.get(&block).copied().unwrap_or(self.floor);
        if revision < newest {
            return false;
        }
        self.heard.insert(block, revision);
        true
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
            if !self.covers(block) && self.nodes.remove(&block).is_some() {
                self.revision += 1;
            }
            return true;
        }
        false
    }

    pub(crate) fn derive(&mut self, block: Uuid, derived: Entries) {
        if let Some(node) = self.nodes.get_mut(&block)
            && node.metadata.derived != derived
        {
            node.metadata.derived.clone_from(&derived);
            self.revision += 1;
        }
        self.derived.insert(block, derived);
    }

    pub(crate) fn forget_derived(&mut self, block: Uuid) {
        self.derived.remove(&block);
    }

    pub(crate) fn refer(&mut self, block: Uuid, references: Vec<Uuid>) {
        if let Some(node) = self.nodes.get_mut(&block)
            && node.references != references
        {
            node.references.clone_from(&references);
            self.revision += 1;
            self.relist(Listing::References(block));
        }
        self.placed(block, &references);
        self.referenced.insert(block, references);
    }

    fn placed(&mut self, parent: Uuid, references: &[Uuid]) {
        let nodes = &self.nodes;
        self.placing.retain(|child| {
            !(references.contains(child)
                && nodes.get(child).and_then(|node| node.parent)
                    == Some(BlockParent::Block(parent)))
        });
    }

    pub(crate) fn forget_references(&mut self, block: Uuid) {
        self.referenced.remove(&block);
    }

    fn insert(&mut self, mut node: Node) {
        if let Some(derived) = self.derived.get(&node.id) {
            node.metadata.derived.clone_from(derived);
        }
        if let Some(references) = self.referenced.get(&node.id) {
            node.references.clone_from(references);
        }
        if self
            .nodes
            .get(&node.id)
            .is_some_and(|known| known.references != node.references)
        {
            self.relist(Listing::References(node.id));
        }
        let references = node.references.clone();
        self.placed(node.id, &references);
        for (listing, listed) in &mut self.listings {
            match member(*listing, &node, &self.nodes) {
                Some(true) => listed.members.insert(node.id),
                Some(false) => listed.members.remove(&node.id),
                None => continue,
            };
        }
        if !self.covers(node.id) {
            if self.nodes.remove(&node.id).is_some() {
                self.revision += 1;
            }
            return;
        }
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

    pub(crate) fn forget(&mut self, block: Uuid) {
        for listed in self.listings.values_mut() {
            listed.members.remove(&block);
        }
        if self.nodes.remove(&block).is_some() {
            self.revision += 1;
        }
    }

    pub(crate) fn get(&self, block: Uuid) -> Option<&Node> {
        self.nodes.get(&block)
    }

    pub(crate) fn query(&self, query: Query) -> Vec<Node> {
        let mut found: Vec<Node> = match query {
            Query::Roots => self.with_parent(BlockParent::Root),
            Query::Detached => self
                .nodes
                .values()
                .filter(|node| node.parent.is_some() && !self.holds(node))
                .cloned()
                .collect(),
            Query::Shared => self
                .listings
                .get(&Listing::Shared)
                .into_iter()
                .flat_map(|listed| listed.members.iter())
                .filter_map(|block| self.nodes.get(block))
                .filter(|node| node.access.can_view())
                .cloned()
                .collect(),
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
            Query::OpenSettings(block) => self
                .listings
                .get(&Listing::OpenSettings(block))
                .into_iter()
                .flat_map(|listed| listed.members.iter())
                .filter_map(|block| self.nodes.get(block))
                .filter(|node| node.access.can_view())
                .cloned()
                .collect(),
            Query::Subtree(root) => self
                .subtree(root)
                .into_iter()
                .filter_map(|block| self.nodes.get(&block).cloned())
                .collect(),
        };
        found.sort_by_key(|node| node.id);
        found
    }

    pub(crate) fn scope_of(&self, block: Uuid) -> Option<Scope> {
        let mut seen = 0;
        let mut current = self.nodes.get(&block)?.parent.and_then(BlockParent::block);
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
            current = node.parent.and_then(BlockParent::block);
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
            Some(node) if node.metadata.get(&be_block::LOCAL_ID) == Some(local) => copy,
            _ => local,
        }
    }

    pub(crate) fn subtree(&self, root: Uuid) -> Vec<Uuid> {
        let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for node in self.nodes.values() {
            if let Some(BlockParent::Block(parent)) = node.parent {
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

    pub(crate) fn placing(&mut self, block: Uuid, placing: bool) {
        let changed = match placing {
            true => self.placing.insert(block),
            false => self.placing.remove(&block),
        };
        if changed {
            self.revision += 1;
        }
    }

    fn holds(&self, node: &Node) -> bool {
        if self.placing.contains(&node.id) {
            return true;
        }
        match node.parent {
            Some(BlockParent::Root) | None => true,
            Some(BlockParent::Detached) => false,
            Some(BlockParent::Block(parent)) => match self.nodes.get(&parent) {
                Some(parent) if parent.access.can_view() => parent.references.contains(&node.id),
                _ => node.held,
            },
        }
    }

    pub(crate) fn deleted_root(&self, block: Uuid) -> Option<Uuid> {
        let mut current = block;
        for _ in 0..=self.nodes.len() {
            let node = self.nodes.get(&current)?;
            if !self.holds(node) {
                return Some(current);
            }
            match node.parent? {
                BlockParent::Block(parent) => current = parent,
                BlockParent::Root | BlockParent::Detached => return None,
            }
        }
        None
    }

    fn with_parent(&self, parent: BlockParent) -> Vec<Node> {
        self.nodes
            .values()
            .filter(|node| node.parent == Some(parent))
            .cloned()
            .collect()
    }

    fn ancestors(&self, block: Uuid) -> Vec<Node> {
        let mut chain = Vec::new();
        let mut current = self
            .nodes
            .get(&block)
            .and_then(|node| node.parent.and_then(BlockParent::block));
        while let Some(parent) = current {
            let Some(node) = self.nodes.get(&parent) else {
                break;
            };
            if chain.iter().any(|seen: &Node| seen.id == parent) {
                break;
            }
            chain.push(node.clone());
            current = node.parent.and_then(BlockParent::block);
        }
        chain.reverse();
        chain
    }
}

fn member(listing: Listing, node: &Node, nodes: &HashMap<Uuid, Node>) -> Option<bool> {
    match listing {
        Listing::Children(parent) => Some(node.parent == Some(parent)),
        Listing::Deleted => Some(node.parent.is_some() && !node.held),
        Listing::Block(block) => Some(block == node.id),
        Listing::Backrefs(block) => Some(node.references.contains(&block)),
        Listing::References(block) => nodes
            .get(&block)
            .map(|referrer| referrer.references.contains(&node.id)),
        Listing::Shared | Listing::OpenSettings(_) => None,
    }
}

#[cfg(test)]
mod tests;
