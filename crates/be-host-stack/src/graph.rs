use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use be_block_commit::CommitId;
use be_block_content::{BlockMetadata, Entries};
use be_block_graph::{Access, BlockParent};
pub use be_block_protocol::Listing;
use be_block_repository::{local_id, masked, scope_mask};
use be_sdk_api::BlockIdRole;
use uuid::Uuid;

use crate::diff::Diff;

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[cfg_attr(not(test), derive(Clone))]
pub struct Node {
    pub id: Uuid,
    pub content_type: Uuid,
    pub author: Option<Uuid>,
    pub parent: Option<BlockParent>,
    pub access: Access,
    pub references: Vec<Uuid>,
    pub metadata: BlockMetadata,
    pub head: Option<CommitId>,
    pub locked: bool,
    pub held: bool,
    pub sets_access: bool,
    /// Bytes the server keeps for this block alone, and with every block below it.
    pub size: u64,
    pub tree_size: u64,
}

#[cfg(test)]
thread_local! {
    /// How many nodes this thread copied, for a test to tell that a change copies what it
    /// changes, not the graph a `Seen` handed over shares. Per thread: tests run side by side.
    pub(crate) static NODES_COPIED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
impl Clone for Node {
    fn clone(&self) -> Self {
        NODES_COPIED.set(NODES_COPIED.get() + 1);
        Self {
            id: self.id,
            content_type: self.content_type,
            author: self.author,
            parent: self.parent,
            access: self.access,
            references: self.references.clone(),
            metadata: self.metadata.clone(),
            head: self.head,
            locked: self.locked,
            held: self.held,
            sets_access: self.sets_access,
            size: self.size,
            tree_size: self.tree_size,
        }
    }
}

impl Node {
    /// An account's Unfiled, which holds its children without listing them.
    pub fn unfiled(&self) -> bool {
        self.content_type == be_block_graph::UNFILED_TYPE
    }

    pub fn contents(&self) -> Vec<Uuid> {
        let described = self.metadata.references();
        self.references
            .iter()
            .filter(|reference| !described.contains(reference))
            .copied()
            .collect()
    }
}

fn with_described(mut references: Vec<Uuid>, metadata: &BlockMetadata) -> Vec<Uuid> {
    for described in metadata.references() {
        if !references.contains(&described) {
            references.push(described);
        }
    }
    references
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scope {
    pub checkout: Uuid,
    pub mask: u128,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Query {
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

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub enum Owner {
    Host,
    Open,
    /// The indexes searches read and the blocks they found (`crate::search`).
    Search,
    Version,
    Organization,
    Editor(u64),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
enum Fetch {
    #[default]
    Wanted,
    Fetching,
    Loaded,
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
struct Listed {
    members: imbl::OrdSet<Uuid>,
    fetch: Fetch,
    /// Asked for again while a fetch was under way, whose answer may be from before the change
    /// that asked: fetched once more when it lands (`relist`).
    again: bool,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct Graph {
    /// Counts the changes to the mirror, for the stack's own work to tell whether it moved;
    /// the host hears of each one as a change notice (`Changes::graph`).
    pub revision: u64,
    /// Persistent maps of shared nodes, so a change copies the path to what it changes rather
    /// than the whole graph a `Seen` handed over shares.
    nodes: imbl::OrdMap<Uuid, Arc<Node>>,
    wants: HashMap<Owner, Vec<Query>>,
    listings: imbl::OrdMap<Listing, Listed>,
    unlisted: Vec<Listing>,
    heard: imbl::OrdMap<Uuid, u64>,
    floor: u64,
    pending: imbl::OrdMap<Uuid, u32>,
    /// Blocks made here that the drain has not sent yet (`made`): no listing the server answers
    /// can hold one, so it stays a member of the listings it belongs in until the drain sends
    /// its creation (`relist_around`), and they are asked for again.
    unpublished: imbl::OrdSet<Uuid>,
    /// Nodes added, changed or removed since the worker last took them (`take_nodes_moved`).
    #[serde(skip)]
    nodes_moved: imbl::OrdSet<Uuid>,
    /// Nodes a change copied because a `Seen` handed over still shared them (`node_mut`), for a
    /// test to count.
    #[cfg(test)]
    #[serde(skip)]
    pub(crate) copied_on_write: u64,
    derived: imbl::OrdMap<Uuid, Entries>,
    referenced: imbl::OrdMap<Uuid, Vec<Uuid>>,
    /// The head an open block's replica last reported, which what the server lists of it does not
    /// outrun: a listing or an event made before the commit reached the server says the head
    /// it had, and the replica says nothing more until the head changes again.
    reported: imbl::OrdMap<Uuid, CommitId>,
    placing: std::collections::HashSet<Uuid>,
    /// The last children whose parent's references took them in (`placed`), with that parent,
    /// so a child that drops out of them again while it still names the parent is noticed
    /// (`dropped`): a block that then reads as deleted.
    #[serde(skip)]
    placed_recently: std::collections::VecDeque<(Uuid, Uuid)>,
    /// Children that dropped out of their parent's references after they were placed there, for
    /// the worker to log with what it knows of the parent (`take_dropped`).
    #[serde(skip)]
    dropped: Vec<Dropped>,
    /// Blocks a listing's answer left out while this device held them as belonging there, for
    /// the worker to log (`take_unlisted_held`): what makes a list show one entry fewer than it
    /// should.
    #[serde(skip)]
    unlisted_held: Vec<UnlistedHeld>,
}

/// What the page reads of the graph, by what changed (`diff::Diff`). What only the worker
/// reads, the moved nodes it works reach out again for and what it logs, stays behind.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct GraphDelta {
    revision: Option<u64>,
    nodes: Option<<imbl::OrdMap<Uuid, Arc<Node>> as Diff>::Delta>,
    wants: Option<HashMap<Owner, Vec<Query>>>,
    listings: Option<<imbl::OrdMap<Listing, Listed> as Diff>::Delta>,
    unlisted: Option<Vec<Listing>>,
    heard: Option<<imbl::OrdMap<Uuid, u64> as Diff>::Delta>,
    floor: Option<u64>,
    pending: Option<<imbl::OrdMap<Uuid, u32> as Diff>::Delta>,
    unpublished: Option<<imbl::OrdSet<Uuid> as Diff>::Delta>,
    derived: Option<<imbl::OrdMap<Uuid, Entries> as Diff>::Delta>,
    referenced: Option<<imbl::OrdMap<Uuid, Vec<Uuid>> as Diff>::Delta>,
    reported: Option<<imbl::OrdMap<Uuid, CommitId> as Diff>::Delta>,
    placing: Option<std::collections::HashSet<Uuid>>,
}

fn changed<T: Clone + PartialEq>(old: &T, new: &T) -> Option<T> {
    (old != new).then(|| new.clone())
}

impl Diff for Graph {
    type Delta = GraphDelta;

    fn diff(old: &Self, new: &Self) -> Option<GraphDelta> {
        // Every field named, so a new one fails to compile until it is placed in the delta or
        // left with the worker.
        let Graph {
            revision,
            nodes,
            wants,
            listings,
            unlisted,
            heard,
            floor,
            pending,
            unpublished,
            nodes_moved: _,
            #[cfg(test)]
                copied_on_write: _,
            derived,
            referenced,
            reported,
            placing,
            placed_recently: _,
            dropped: _,
            unlisted_held: _,
        } = new;
        let delta = GraphDelta {
            revision: changed(&old.revision, revision),
            nodes: Diff::diff(&old.nodes, nodes),
            wants: changed(&old.wants, wants),
            listings: Diff::diff(&old.listings, listings),
            unlisted: changed(&old.unlisted, unlisted),
            heard: Diff::diff(&old.heard, heard),
            floor: changed(&old.floor, floor),
            pending: Diff::diff(&old.pending, pending),
            unpublished: Diff::diff(&old.unpublished, unpublished),
            derived: Diff::diff(&old.derived, derived),
            referenced: Diff::diff(&old.referenced, referenced),
            reported: Diff::diff(&old.reported, reported),
            placing: changed(&old.placing, placing),
        };
        let GraphDelta {
            revision,
            nodes,
            wants,
            listings,
            unlisted,
            heard,
            floor,
            pending,
            unpublished,
            derived,
            referenced,
            reported,
            placing,
        } = &delta;
        let changed = revision.is_some()
            || nodes.is_some()
            || wants.is_some()
            || listings.is_some()
            || unlisted.is_some()
            || heard.is_some()
            || floor.is_some()
            || pending.is_some()
            || unpublished.is_some()
            || derived.is_some()
            || referenced.is_some()
            || reported.is_some()
            || placing.is_some();
        changed.then_some(delta)
    }

    fn apply(&mut self, delta: GraphDelta) {
        let GraphDelta {
            revision,
            nodes,
            wants,
            listings,
            unlisted,
            heard,
            floor,
            pending,
            unpublished,
            derived,
            referenced,
            reported,
            placing,
        } = delta;
        if let Some(revision) = revision {
            self.revision = revision;
        }
        if let Some(nodes) = nodes {
            self.nodes.apply(nodes);
        }
        if let Some(wants) = wants {
            self.wants = wants;
        }
        if let Some(listings) = listings {
            self.listings.apply(listings);
        }
        if let Some(unlisted) = unlisted {
            self.unlisted = unlisted;
        }
        if let Some(heard) = heard {
            self.heard.apply(heard);
        }
        if let Some(floor) = floor {
            self.floor = floor;
        }
        if let Some(pending) = pending {
            self.pending.apply(pending);
        }
        if let Some(unpublished) = unpublished {
            self.unpublished.apply(unpublished);
        }
        if let Some(derived) = derived {
            self.derived.apply(derived);
        }
        if let Some(referenced) = referenced {
            self.referenced.apply(referenced);
        }
        if let Some(reported) = reported {
            self.reported.apply(reported);
        }
        if let Some(placing) = placing {
            self.placing = placing;
        }
    }
}

/// Equal in what the page reads of it (`GraphDelta`), which the debug check of the updates
/// compares (`seen::Handed::check`).
impl PartialEq for Graph {
    fn eq(&self, other: &Self) -> bool {
        let Graph {
            revision,
            nodes,
            wants,
            listings,
            unlisted,
            heard,
            floor,
            pending,
            unpublished,
            nodes_moved: _,
            #[cfg(test)]
                copied_on_write: _,
            derived,
            referenced,
            reported,
            placing,
            placed_recently: _,
            dropped: _,
            unlisted_held: _,
        } = self;
        *revision == other.revision
            && *nodes == other.nodes
            && *wants == other.wants
            && *listings == other.listings
            && *unlisted == other.unlisted
            && *heard == other.heard
            && *floor == other.floor
            && *pending == other.pending
            && *unpublished == other.unpublished
            && *derived == other.derived
            && *referenced == other.referenced
            && *reported == other.reported
            && *placing == other.placing
    }
}

/// How many placed children `Graph::placed_recently` remembers.
const PLACED_REMEMBERED: usize = 64;

/// A child that dropped out of its parent's references after they had taken it in, while it
/// still names that parent: it reads as deleted (`Graph::dropped`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Dropped {
    pub child: Uuid,
    pub parent: Uuid,
    /// What set the parent's references without it: its open replica's report, or a listing.
    pub by: crate::seen::Words,
}

/// Blocks the server's answer to a listing left out while this device held each as belonging
/// in it, with the revision the answer was read at and whether it answered a request made
/// before a change asked for the listing again.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UnlistedHeld {
    pub listing: Listing,
    pub blocks: Vec<Uuid>,
    pub revision: u64,
    pub stale: bool,
}

impl Graph {
    fn moved(&mut self) {
        self.revision += 1;
        crate::changes::graph();
    }

    pub fn want(&mut self, owner: Owner, queries: Vec<Query>) {
        match queries.is_empty() {
            true => self.wants.remove(&owner),
            false => self.wants.insert(owner, queries),
        };
        self.refresh_listings();
    }

    pub fn wanted(&self, owner: Owner) -> &[Query] {
        self.wants.get(&owner).map_or(&[], Vec::as_slice)
    }

    pub fn refresh_listings(&mut self) {
        let mut desired = HashSet::new();
        for queries in self.wants.values() {
            for query in queries {
                self.listings_for(*query, &mut desired);
            }
        }
        let before = self.listings.len();
        let dropped: Vec<Listing> = self
            .listings
            .keys()
            .filter(|listing| !desired.contains(*listing))
            .copied()
            .collect();
        for listing in dropped {
            if let Some(listed) = self.listings.remove(&listing)
                && listed.fetch != Fetch::Wanted
            {
                self.unlisted.push(listing);
            }
        }
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
                    again: false,
                },
            );
            changed = true;
        }
        if changed {
            self.prune();
            self.moved();
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

    #[cfg(any(test, feature = "testing"))]
    pub fn listings_of(&self, query: Query) -> HashSet<Listing> {
        let mut listings = HashSet::new();
        self.listings_for(query, &mut listings);
        listings
    }

    /// Whether `take_wanted` or `take_unlisted` has anything to take: asked first, since taking
    /// changes the graph, which copies it while a `Seen` handed over shares it.
    pub fn has_listings_to_take(&self) -> bool {
        !self.unlisted.is_empty()
            || self
                .listings
                .values()
                .any(|listed| listed.fetch == Fetch::Wanted)
    }

    pub fn take_wanted(&mut self) -> Vec<Listing> {
        let mut found = Vec::new();
        for (listing, listed) in &self.listings {
            if listed.fetch == Fetch::Wanted {
                found.push(*listing);
            }
        }
        for listing in &found {
            if let Some(listed) = self.listings.get_mut(listing) {
                listed.fetch = Fetch::Fetching;
            }
        }
        found
    }

    pub fn take_unlisted(&mut self) -> Vec<Listing> {
        std::mem::take(&mut self.unlisted)
    }

    pub fn listed(&mut self, listing: Listing, nodes: Vec<Node>, revision: u64) {
        let Some(listed) = self.listings.get_mut(&listing) else {
            return;
        };
        let stale = std::mem::take(&mut listed.again);
        listed.fetch = match stale {
            true => Fetch::Wanted,
            false => Fetch::Loaded,
        };
        // An answer to a request made before a change that asked for the listing again
        // cannot know of what that change added: what was listed stays until the answer
        // to the request after it, so a block made or moved here does not leave the
        // graph between its change settling and that answer.
        let before = std::mem::replace(
            &mut listed.members,
            nodes.iter().map(|node| node.id).collect(),
        );
        let kept = before.iter().filter(|_| stale);
        listed.members.extend(
            kept.chain(self.unpublished.iter())
                .copied()
                .filter(|block| {
                    self.nodes
                        .get(block)
                        .is_some_and(|node| member(listing, node, &self.nodes) == Some(true))
                }),
        );
        let described = match listing {
            Listing::References(block) => self.referenced.contains_key(&block),
            _ => false,
        };
        listed.members.extend(
            self.nodes
                .values()
                .filter(|node| {
                    described
                        || self.pending.contains_key(&node.id)
                        || self
                            .heard
                            .get(&node.id)
                            .is_some_and(|heard| *heard > revision)
                })
                .filter(|node| member(listing, node, &self.nodes) == Some(true))
                .map(|node| node.id),
        );
        let unlisted: Vec<Uuid> = before
            .into_iter()
            .filter(|block| !listed.members.contains(block))
            .filter(|block| {
                self.nodes
                    .get(block)
                    .is_some_and(|node| member(listing, node, &self.nodes) == Some(true))
            })
            .collect();
        if !unlisted.is_empty() {
            self.unlisted_held.push(UnlistedHeld {
                listing,
                blocks: unlisted,
                revision,
                stale,
            });
        }
        self.floor = self.floor.max(revision);
        for node in nodes {
            if self.hear(node.id, revision) && !self.pending.contains_key(&node.id) {
                self.insert(node);
            }
        }
        self.prune();
        self.moved();
        self.refresh_listings();
    }

    pub fn failed(&mut self, listing: Listing, error: String) {
        let Some(listed) = self.listings.get_mut(&listing) else {
            return;
        };
        // Asked for again while it was under way: the change that asked may answer it now.
        listed.fetch = match std::mem::take(&mut listed.again) {
            true => Fetch::Wanted,
            false => Fetch::Failed(error),
        };
        self.moved();
    }

    /// A listing whose request never got an answer is asked for again.
    pub fn unfetch(&mut self, listing: Listing) {
        if let Some(listed) = self.listings.get_mut(&listing)
            && listed.fetch == Fetch::Fetching
        {
            listed.fetch = Fetch::Wanted;
        }
    }

    pub fn relist(&mut self, listing: Listing) {
        let Some(listed) = self.listings.get_mut(&listing) else {
            return;
        };
        match listed.fetch {
            Fetch::Loaded | Fetch::Failed(_) => listed.fetch = Fetch::Wanted,
            Fetch::Fetching => listed.again = true,
            Fetch::Wanted => {}
        }
    }

    /// What the server answers of `block`'s place changed (the drain sent its creation or its
    /// move): the listings it is in or about are asked for again.
    pub fn relist_around(&mut self, block: Uuid, parent: Option<BlockParent>) {
        self.unpublished.remove(&block);
        if let Some(parent) = parent {
            self.relist(Listing::Children(parent));
        }
        for listing in [
            Listing::Block(block),
            Listing::OpenSettings(block),
            Listing::References(block),
            Listing::Backrefs(block),
        ] {
            self.relist(listing);
        }
    }

    pub fn relist_open_settings(&mut self) {
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

    pub fn relist_owner(&mut self, owner: Owner) {
        let mut held = HashSet::new();
        for query in self.wanted(owner) {
            self.listings_for(*query, &mut held);
        }
        for listing in held {
            self.relist(listing);
        }
    }

    pub fn relist_all(&mut self) {
        self.heard.clear();
        self.floor = 0;
        let fetched: Vec<Listing> = self
            .listings
            .iter()
            .filter(|(_, listed)| listed.fetch != Fetch::Wanted)
            .map(|(listing, _)| *listing)
            .collect();
        for listing in fetched {
            if let Some(listed) = self.listings.get_mut(&listing) {
                listed.fetch = Fetch::Wanted;
            }
        }
    }

    pub fn ready(&self, query: Query) -> bool {
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

    pub fn failure(&self, query: Query) -> Option<String> {
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

    /// How many listings failed to load and are not asked for again until something relists them.
    #[cfg(any(test, feature = "testing"))]
    pub fn failed_listings(&self) -> usize {
        self.listings
            .values()
            .filter(|listed| matches!(listed.fetch, Fetch::Failed(_)))
            .count()
    }

    pub fn fetching(&self) -> bool {
        self.listings
            .values()
            .any(|listed| matches!(listed.fetch, Fetch::Wanted | Fetch::Fetching))
    }

    pub fn covers(&self, block: Uuid) -> bool {
        self.pending.contains_key(&block)
            || self.placing.contains(&block)
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
            self.remove_node(block);
            self.moved();
        }
    }

    pub fn put(&mut self, node: Node, revision: u64) {
        // A block whose own listing failed, or may (asked for offline before this device made
        // it, or heard of it), answers it now.
        let listing = Listing::Block(node.id);
        if self
            .listings
            .get(&listing)
            .is_some_and(|listed| matches!(listed.fetch, Fetch::Failed(_) | Fetch::Fetching))
        {
            self.relist(listing);
        }
        if self.hear(node.id, revision) && !self.pending.contains_key(&node.id) {
            self.insert(node);
        }
    }

    /// Puts a node read again from what this device already heard of it (keys arrived that
    /// open it), as of `revision`; one it does not hold, has a change pending on, or heard of
    /// at a newer revision since is left as it is.
    pub fn reread(&mut self, node: Node, revision: u64) {
        let newest = self.heard.get(&node.id).copied().unwrap_or(self.floor);
        if revision >= newest
            && self.nodes.contains_key(&node.id)
            && !self.pending.contains_key(&node.id)
        {
            self.insert(node);
        }
    }

    pub fn remove(&mut self, block: Uuid, revision: u64) {
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

    /// A block made here, kept until the drain sends it: what the server lists before then
    /// cannot hold it (`unpublished`).
    pub fn made(&mut self, block: Uuid) {
        self.unpublished.insert(block);
    }

    pub fn change(&mut self, node: Node) {
        *self.pending.entry(node.id).or_default() += 1;
        self.insert(node);
    }

    /// A node to change, copied first if a `Seen` handed over shares it: the persistent map
    /// copies the path to it, and the node itself only when shared.
    /// Removes a node, answering whether it was there.
    fn remove_node(&mut self, block: Uuid) -> bool {
        let removed = self.nodes.remove(&block).is_some();
        if removed {
            self.nodes_moved.insert(block);
        }
        removed
    }

    /// Takes the nodes added, changed or removed since the last take (`nodes_moved`), for the
    /// worker to work out again what it derives from them alone. Asks first, so a graph with
    /// nothing moved is not written.
    pub(crate) fn take_nodes_moved(&mut self) -> imbl::OrdSet<Uuid> {
        if self.nodes_moved.is_empty() {
            return imbl::OrdSet::new();
        }
        std::mem::take(&mut self.nodes_moved)
    }

    fn node_mut(&mut self, block: Uuid) -> Option<&mut Node> {
        if !self.nodes.contains_key(&block) {
            return None;
        }
        self.nodes_moved.insert(block);
        let node = self.nodes.get_mut(&block)?;
        #[cfg(test)]
        if Arc::strong_count(node) > 1 {
            self.copied_on_write += 1;
        }
        Some(Arc::make_mut(node))
    }

    pub fn update(&mut self, block: Uuid, change: impl FnOnce(&mut Node)) {
        if self.nodes.contains_key(&block) {
            *self.pending.entry(block).or_default() += 1;
            if let Some(node) = self.node_mut(block) {
                change(node);
            }
            self.moved();
        }
    }

    pub fn settle(&mut self, block: Uuid) -> bool {
        let Some(count) = self.pending.get_mut(&block) else {
            return true;
        };
        *count -= 1;
        if *count == 0 {
            self.pending.remove(&block);
            if !self.covers(block) && self.remove_node(block) {
                self.moved();
            }
            return true;
        }
        false
    }

    /// Whether `derive` would change nothing.
    pub fn derives(&self, block: Uuid, derived: &Entries) -> bool {
        self.derived.get(&block) == Some(derived)
            && self
                .nodes
                .get(&block)
                .is_none_or(|node| node.metadata.derived == *derived)
    }

    pub fn derive(&mut self, block: Uuid, derived: Entries) {
        if self.derives(block, &derived) {
            return;
        }
        if self
            .nodes
            .get(&block)
            .is_some_and(|node| node.metadata.derived != derived)
            && let Some(node) = self.node_mut(block)
        {
            node.metadata.derived.clone_from(&derived);
            self.moved();
        }
        self.derived.insert(block, derived);
    }

    pub fn forget_derived(&mut self, block: Uuid) {
        self.derived.remove(&block);
    }

    /// Whether `refer` would change nothing: the replica reports the references it reported
    /// last, the node and its references' listing hold them, and no child waits to be placed.
    pub fn refers(&self, block: Uuid, references: &[Uuid]) -> bool {
        self.referenced.get(&block).map(Vec::as_slice) == Some(references)
            && self.nodes.get(&block).is_none_or(|node| {
                node.references == with_described(references.to_vec(), &node.metadata)
            })
            && self
                .listings
                .get(&Listing::References(block))
                .is_none_or(|listed| {
                    references.iter().all(|reference| {
                        !self.nodes.contains_key(reference) || listed.members.contains(reference)
                    })
                })
            && !references
                .iter()
                .any(|reference| self.placing.contains(reference))
    }

    pub fn refer(&mut self, block: Uuid, references: Vec<Uuid>) {
        let all = self
            .nodes
            .get(&block)
            .map(|node| with_described(references.clone(), &node.metadata))
            .filter(|all| {
                self.nodes
                    .get(&block)
                    .is_some_and(|node| node.references != *all)
            });
        if let Some(all) = all
            && let Some(node) = self.node_mut(block)
        {
            node.references = all;
            self.moved();
            self.relist(Listing::References(block));
            self.check_dropped(block, "its open replica's report");
        }
        if let Some(listed) = self.listings.get_mut(&Listing::References(block)) {
            listed.members.extend(
                references
                    .iter()
                    .copied()
                    .filter(|reference| self.nodes.contains_key(reference)),
            );
        }
        self.placed(block, &references);
        self.referenced.insert(block, references);
    }

    fn placed(&mut self, parent: Uuid, references: &[Uuid]) {
        let nodes = &self.nodes;
        let references: std::collections::HashSet<&Uuid> = references.iter().collect();
        let mut taken = Vec::new();
        self.placing.retain(|child| {
            let placed = references.contains(child)
                && nodes.get(child).and_then(|node| node.parent)
                    == Some(BlockParent::Block(parent));
            if placed {
                taken.push(*child);
            }
            !placed
        });
        for child in taken {
            if self.placed_recently.len() == PLACED_REMEMBERED {
                self.placed_recently.pop_front();
            }
            self.placed_recently.push_back((child, parent));
        }
    }

    /// Notes the children placed in `parent` that its references, just set `by` something, no
    /// longer hold though they still name it. Children that moved elsewhere are forgotten.
    fn check_dropped(&mut self, parent: Uuid, by: &'static str) {
        let Some(references) = self.nodes.get(&parent).map(|node| {
            node.references
                .iter()
                .copied()
                .collect::<std::collections::HashSet<Uuid>>()
        }) else {
            return;
        };
        let nodes = &self.nodes;
        let dropped = &mut self.dropped;
        self.placed_recently.retain(|&(child, placed_in)| {
            if placed_in != parent {
                return true;
            }
            let names_it =
                nodes.get(&child).and_then(|node| node.parent) == Some(BlockParent::Block(parent));
            if names_it && !references.contains(&child) {
                dropped.push(Dropped {
                    child,
                    parent,
                    by: crate::seen::Words::Borrowed(by),
                });
                return false;
            }
            names_it
        });
    }

    /// What listings' answers left out of what this device held since this was last asked
    /// (`UnlistedHeld`).
    pub fn take_unlisted_held(&mut self) -> Vec<UnlistedHeld> {
        std::mem::take(&mut self.unlisted_held)
    }

    /// The children that dropped out of their parent's references since this was last asked.
    pub(crate) fn has_nodes_moved(&self) -> bool {
        !self.nodes_moved.is_empty()
    }

    pub fn has_dropped(&self) -> bool {
        !self.dropped.is_empty()
    }

    pub fn take_dropped(&mut self) -> Vec<Dropped> {
        std::mem::take(&mut self.dropped)
    }

    pub fn forget_references(&mut self, block: Uuid) {
        self.referenced.remove(&block);
    }

    fn insert(&mut self, mut node: Node) {
        if let Some(derived) = self.derived.get(&node.id) {
            node.metadata.derived.clone_from(derived);
        }
        if let Some(head) = self.reported.get(&node.id) {
            node.head = Some(*head);
        }
        if let Some(references) = self.referenced.get(&node.id) {
            node.references = with_described(references.clone(), &node.metadata);
        }
        let changed_references = self
            .nodes
            .get(&node.id)
            .is_some_and(|known| known.references != node.references);
        if changed_references {
            self.relist(Listing::References(node.id));
        }
        let references = node.references.clone();
        self.placed(node.id, &references);
        // Only the listings whose membership changes are written: a persistent map copies what
        // it writes while a `Seen` handed over shares it.
        let moved: Vec<(Listing, bool)> = self
            .listings
            .iter()
            .filter_map(|(listing, listed)| {
                let member = member(*listing, &node, &self.nodes)?;
                (member != listed.members.contains(&node.id)).then_some((*listing, member))
            })
            .collect();
        for (listing, member) in moved {
            if let Some(listed) = self.listings.get_mut(&listing) {
                match member {
                    true => listed.members.insert(node.id),
                    false => listed.members.remove(&node.id),
                };
            }
        }
        if !self.covers(node.id) {
            if self.remove_node(node.id) {
                self.moved();
            }
            return;
        }
        let by = match self.referenced.contains_key(&node.id) {
            true => "its open replica's report",
            false => "a listing",
        };
        let id = node.id;
        if self.nodes.get(&node.id).map(|known| &**known) != Some(&node) {
            self.nodes_moved.insert(node.id);
            self.nodes.insert(node.id, Arc::new(node));
            self.moved();
        }
        if changed_references {
            self.check_dropped(id, by);
        }
    }

    /// The head of an open block's replica, kept over whatever is listed of it until
    /// `forget_reported_head`, and set on its node now or when the node arrives. `None` is no
    /// news: it keeps the head reported before.
    /// Whether `report_head` would change nothing: the replica reports what it reported last,
    /// and the node already has it.
    pub fn reports(&self, block: Uuid, head: Option<CommitId>) -> bool {
        let Some(head) = head else {
            return true;
        };
        self.reported.get(&block) == Some(&head)
            && self
                .nodes
                .get(&block)
                .is_none_or(|node| node.head == Some(head))
    }

    pub fn report_head(&mut self, block: Uuid, head: Option<CommitId>) {
        if let Some(head) = head {
            self.reported.insert(block, head);
        }
        self.set_head(block, head);
    }

    /// A head the outbox published from what this device kept. An open block's replica kept
    /// it and reports its own head, which by the time a drain is answered may already be a
    /// newer commit: only a block no replica reports for follows the drain. With no replica
    /// open, a late drain can still move the head back past a newer head that arrived
    /// meanwhile; telling which is newer would need comparing the commits by ancestry.
    pub fn drained_head(&mut self, block: Uuid, head: CommitId) {
        if !self.reported.contains_key(&block) {
            self.set_head(block, Some(head));
        }
    }

    pub fn forget_reported_head(&mut self, block: Uuid) {
        self.reported.remove(&block);
    }

    /// A head some other writer sets (a settled merge, the head a block remembers) is what the
    /// replica's is now: the overlay follows it, or the next listing would put the old one back.
    pub fn set_head(&mut self, block: Uuid, head: Option<CommitId>) {
        if let Some(head) = head
            && self
                .reported
                .get(&block)
                .is_some_and(|reported| *reported != head)
        {
            self.reported.insert(block, head);
        }
        if head.is_some()
            && self.nodes.get(&block).is_some_and(|node| node.head != head)
            && let Some(node) = self.node_mut(block)
        {
            node.head = head;
            self.moved();
        }
    }

    pub fn forget(&mut self, block: Uuid) {
        self.unpublished.remove(&block);
        let holding: Vec<Listing> = self
            .listings
            .iter()
            .filter(|(_, listed)| listed.members.contains(&block))
            .map(|(listing, _)| *listing)
            .collect();
        for listing in holding {
            if let Some(listed) = self.listings.get_mut(&listing) {
                listed.members.remove(&block);
            }
        }
        if self.remove_node(block) {
            self.moved();
        }
    }

    pub fn get(&self, block: Uuid) -> Option<&Node> {
        self.nodes.get(&block).map(|node| &**node)
    }

    /// Forgets a block's name, as a mirror that has not read a rename yet holds it (a test's
    /// way to hold the mirror back).
    #[cfg(test)]
    pub(crate) fn forget_name(&mut self, block: Uuid) {
        if let Some(node) = self.node_mut(block) {
            node.metadata.remove(&be_block_content::NAME);
        }
    }

    /// Every block the host has loaded, in no order.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values().map(|node| &**node)
    }

    pub fn query(&self, query: Query) -> Vec<Node> {
        let mut found: Vec<Node> = match query {
            Query::Roots => self.with_parent(BlockParent::Root),
            Query::Detached => {
                let mut sets = HashMap::new();
                self.nodes
                    .values()
                    .filter(|node| node.parent.is_some() && !self.holds_in(node, &mut sets))
                    .map(|node| Node::clone(node))
                    .collect()
            }
            Query::Shared => self
                .listings
                .get(&Listing::Shared)
                .into_iter()
                .flat_map(|listed| listed.members.iter())
                .filter_map(|block| self.nodes.get(block))
                .filter(|node| node.access.can_view())
                .map(|node| Node::clone(node))
                .collect(),
            Query::Children(parent) => self.with_parent(BlockParent::Block(parent)),
            Query::References(block) => {
                return self.nodes.get(&block).map_or_else(Vec::new, |node| {
                    node.contents()
                        .iter()
                        .filter_map(|reference| {
                            self.nodes.get(reference).map(|node| Node::clone(node))
                        })
                        .collect()
                });
            }
            Query::Backrefs(block) => self
                .nodes
                .values()
                .filter(|node| node.references.contains(&block))
                .map(|node| Node::clone(node))
                .collect(),
            Query::Parents(block) => return self.ancestors(block),
            Query::Block(block) => {
                return self
                    .nodes
                    .get(&block)
                    .map(|node| Node::clone(node))
                    .into_iter()
                    .collect();
            }
            Query::OpenSettings(block) => self
                .listings
                .get(&Listing::OpenSettings(block))
                .into_iter()
                .flat_map(|listed| listed.members.iter())
                .filter_map(|block| self.nodes.get(block))
                .filter(|node| node.access.can_view())
                .map(|node| Node::clone(node))
                .collect(),
            Query::Subtree(root) => self
                .subtree(root)
                .into_iter()
                .filter_map(|block| self.nodes.get(&block).map(|node| Node::clone(node)))
                .collect(),
        };
        found.sort_by_key(|node| node.id);
        found
    }

    pub fn scope_of(&self, block: Uuid) -> Option<Scope> {
        let mut seen = 0;
        let mut current = self.nodes.get(&block)?.parent.and_then(BlockParent::block);
        while let Some(parent) = current {
            let node = self.nodes.get(&parent)?;
            if node.content_type
                == <be_block_repository::Checkout as be_block_content::Root>::CONTENT_TYPE
            {
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

    pub fn to_local(&self, scope: Scope, real: Uuid) -> Uuid {
        self.nodes
            .get(&real)
            .map_or(real, |node| local_id(real, &node.metadata, scope.mask))
    }

    pub fn to_real(&self, scope: Scope, local: Uuid, role: BlockIdRole) -> Uuid {
        if role == BlockIdRole::Created {
            return local;
        }
        let copy = masked(local, scope.mask);
        match self.nodes.get(&copy) {
            Some(node) if node.metadata.get(&be_block_content::LOCAL_ID) == Some(local) => copy,
            _ => local,
        }
    }

    pub fn subtree(&self, root: Uuid) -> Vec<Uuid> {
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

    pub fn placing(&mut self, block: Uuid, placing: bool) {
        let changed = match placing {
            true => self.placing.insert(block),
            false => self.placing.remove(&block),
        };
        if changed {
            self.moved();
        }
    }

    fn holds(&self, node: &Node) -> bool {
        self.holds_in(node, &mut HashMap::new())
    }

    /// Whether `node`'s parent holds it, with each parent's references made a set once in
    /// `sets`, so asking of every child of a folder of many is not quadratic.
    fn holds_in(
        &self,
        node: &Node,
        sets: &mut HashMap<Uuid, std::collections::HashSet<Uuid>>,
    ) -> bool {
        if self.placing.contains(&node.id) {
            return true;
        }
        match node.parent {
            Some(BlockParent::Root) | None => true,
            Some(BlockParent::Detached) => false,
            Some(BlockParent::Block(parent)) => match self.nodes.get(&parent) {
                Some(parent) if parent.access.can_view() => {
                    parent.unfiled()
                        || sets
                            .entry(parent.id)
                            .or_insert_with(|| parent.references.iter().copied().collect())
                            .contains(&node.id)
                }
                _ => node.held,
            },
        }
    }

    /// Whether any block the mirror knows has `block` as its parent.
    pub fn has_children(&self, block: Uuid) -> bool {
        let parent = BlockParent::Block(block);
        self.nodes.values().any(|node| node.parent == Some(parent))
    }

    /// Whether `block` is `root` or below it, as far as the mirror knows its parents.
    pub fn is_within(&self, block: Uuid, root: Uuid) -> bool {
        let mut current = block;
        for _ in 0..=self.nodes.len() {
            if current == root {
                return true;
            }
            match self.nodes.get(&current).and_then(|node| node.parent) {
                Some(BlockParent::Block(parent)) => current = parent,
                _ => return false,
            }
        }
        false
    }

    pub fn deleted_root(&self, block: Uuid) -> Option<Uuid> {
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
            .map(|node| Node::clone(node))
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
            chain.push(Node::clone(node));
            current = node.parent.and_then(BlockParent::block);
        }
        chain.reverse();
        chain
    }
}

fn member(listing: Listing, node: &Node, nodes: &imbl::OrdMap<Uuid, Arc<Node>>) -> Option<bool> {
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
