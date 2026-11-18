use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use be_block_commit::CommitId;
use be_block_content::{BlockMetadata, Entries};
use be_block_graph::{Access, BlockParent};
use be_block_repository::{local_id, masked, scope_mask};
use be_sdk_api::BlockIdRole;
use uuid::Uuid;

use crate::diff::Diff;

/// What the mirror reads from the server: a listing (`be_block_protocol::Listing`), or one
/// block's own summary.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub enum Listing {
    Children(BlockParent),
    Deleted,
    Shared,
    Backrefs(Uuid),
    Block(Uuid),
    References(Uuid),
    OpenSettings(Uuid),
}

impl Listing {
    /// The server's listing this reads, or none for a block's own summary.
    pub fn served(self) -> Option<be_block_protocol::Listing> {
        Some(match self {
            Self::Children(parent) => be_block_protocol::Listing::Children(parent),
            Self::Deleted => be_block_protocol::Listing::Deleted,
            Self::Shared => be_block_protocol::Listing::Shared,
            Self::Backrefs(block) => be_block_protocol::Listing::Backrefs(block),
            Self::References(block) => be_block_protocol::Listing::References(block),
            Self::OpenSettings(block) => be_block_protocol::Listing::OpenSettings(block),
            Self::Block(_) => return None,
        })
    }
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[cfg_attr(not(test), derive(Clone))]
pub struct Node {
    pub id: Uuid,
    pub content_type: Uuid,
    pub author: Option<Uuid>,
    pub parent: Option<BlockParent>,
    pub access: Access,
    /// What the block's content references, in its order: its open replica's report, or read
    /// from the head (`Graph::read_references`), or for a block with no content yet its
    /// metadata's. None until read, and for a block this account may not read: no summary
    /// names them.
    pub references: Option<Vec<Uuid>>,
    pub metadata: BlockMetadata,
    pub head: Option<CommitId>,
    pub locked: bool,
    pub held: bool,
    pub sets_access: bool,
    /// Bytes the server keeps for this block alone, and with every block below it.
    pub size: u64,
    pub tree_size: u64,
    /// The organization's sequence at the block's last change (`block-summary.changed`).
    pub changed: u64,
    /// The entry point whose tree the mirror follows to hear of the block's changes
    /// (`block-summary.entry`).
    pub entry: Option<Uuid>,
    /// How many things in the block's content the account has not read, for a type that counts
    /// them (`crate::unread`): no summary says it, so a node read again keeps the count it had
    /// until it is counted again.
    pub unread: u32,
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
            changed: self.changed,
            entry: self.entry,
            unread: self.unread,
        }
    }
}

impl Node {
    /// An account's Unfiled, which holds its children without listing them.
    pub fn unfiled(&self) -> bool {
        self.content_type == be_block_graph::UNFILED_TYPE
    }

    /// What the block's content holds, its metadata's references aside: none while its
    /// references are not read.
    pub fn contents(&self) -> Vec<Uuid> {
        let described = self.metadata.references();
        self.references
            .iter()
            .flatten()
            .filter(|reference| !described.contains(reference))
            .copied()
            .collect()
    }

    /// Whether the block's content references `block`: none while its references are not read.
    pub fn refers_to(&self, block: Uuid) -> Option<bool> {
        self.references
            .as_ref()
            .map(|references| references.contains(&block))
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
    /// The server answered it once: a fetch since reads it again (`Graph::refreshing`), while
    /// what it holds keeps showing.
    answered: bool,
}

/// An entry point the mirror follows because some node it holds lies in its tree
/// (`Node::entry`): read with `query.tree` from `cursor` when the server says its tree
/// advanced (`Graph::advanced`).
#[derive(Clone, Debug, Default, PartialEq)]
struct Follow {
    /// The nodes held whose entry it is; at none, it is unfollowed.
    nodes: u32,
    /// The sequence its tree was read to.
    cursor: u64,
    fetch: Fetch,
    /// Advanced while a read was under way: read once more when it lands.
    again: bool,
}

/// What `member` reads of the graph: borrowed field by field, so a listing can be changed while
/// its members are worked out.
struct Known<'a> {
    nodes: &'a imbl::OrdMap<Uuid, Arc<Node>>,
    referenced: &'a imbl::OrdMap<Uuid, Vec<Uuid>>,
    read: &'a imbl::OrdMap<Uuid, (CommitId, Vec<Uuid>)>,
    organization_settings: Option<Uuid>,
}

macro_rules! known {
    ($graph:expr) => {
        Known {
            nodes: &$graph.nodes,
            referenced: &$graph.referenced,
            read: &$graph.read,
            organization_settings: $graph.organization_settings,
        }
    };
}

impl Known<'_> {
    /// Whether `node`'s references are those of what it holds now: its open replica's report,
    /// what was read at its head, or with no content yet its metadata's. At a new head the
    /// references read at the last one show until it is read, but say nothing of what it
    /// points at now.
    fn current(&self, node: &Node) -> bool {
        node.references.is_some()
            && (self.referenced.contains_key(&node.id)
                || node.head.is_none()
                || self
                    .read
                    .get(&node.id)
                    .is_some_and(|(at, _)| node.head == Some(*at)))
    }
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
    /// The newest `changed` heard of each block, so an older answer never undoes a newer one.
    heard: imbl::OrdMap<Uuid, u64>,
    pending: imbl::OrdMap<Uuid, u32>,
    /// The entry points followed, which only the worker reads.
    #[serde(skip)]
    follows: imbl::OrdMap<Uuid, Follow>,
    /// Entry points no node lies in any more, to unfollow.
    #[serde(skip)]
    unfollowed: Vec<Uuid>,
    /// Blocks made here that the drain has not sent yet (`made`): no listing the server answers
    /// can hold one, so it stays a member of the listings it belongs in until the drain sends
    /// its creation (`relist_around`), and they are asked for again.
    unpublished: imbl::OrdSet<Uuid>,
    /// Nodes added, changed or removed since the worker last took them (`take_nodes_moved`).
    #[serde(skip)]
    nodes_moved: imbl::OrdSet<Uuid>,
    /// The same, since the unread counts last took them (`take_to_count`).
    #[serde(skip)]
    to_count: imbl::OrdSet<Uuid>,
    /// Nodes a change copied because a `Seen` handed over still shared them (`node_mut`), for a
    /// test to count.
    #[cfg(test)]
    #[serde(skip)]
    pub(crate) copied_on_write: u64,
    derived: imbl::OrdMap<Uuid, Entries>,
    referenced: imbl::OrdMap<Uuid, Vec<Uuid>>,
    /// What a block's content references, read from a head (`read_references`) for a block no
    /// replica has open, or kept from its replica's last report as it closed: a node at that
    /// head takes them, since no summary names them.
    read: imbl::OrdMap<Uuid, (CommitId, Vec<Uuid>)>,
    /// Blocks whose head's references the worker is reading (`take_to_read`).
    #[serde(skip)]
    reading: imbl::OrdSet<Uuid>,
    /// Whether a node, a want or a read changed since `take_to_read` last looked, so a pass of
    /// the worker with nothing new to read does not scan the nodes.
    #[serde(skip)]
    to_read_changed: bool,
    /// Reads of a head that failed for a reason that may pass (a fetch cut off, a timeout), with
    /// how many times, and whether it waits for the next answer from the server (a listing or a
    /// tree's page) before it is tried again: at `READ_ATTEMPTS` it is `unreadable`.
    #[serde(skip)]
    retrying: HashMap<Uuid, Retry>,
    /// Heads whose references could not be read (offline and not kept on this device, or not
    /// opened by a key yet): not read again at that head until a new connection (`relist_all`)
    /// or new keys (`keys_changed`). What was known before shows meanwhile, or else what the
    /// server's listing holds.
    unreadable: imbl::OrdMap<Uuid, CommitId>,
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
    /// The open organization's settings block (`be_block::organization_settings`), which lies
    /// at the root but is no root block: the server's `Children(Root)` leaves it out, and so do
    /// the mirror's listing and `Query::Roots` (`organized`).
    organization_settings: Option<Uuid>,
}

/// What the page reads of the graph, by what changed (`diff::Diff`). What only the worker
/// reads, the moved nodes it works reach out again for and what it logs, stays behind.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct GraphDelta {
    revision: Option<u64>,
    nodes: Option<<imbl::OrdMap<Uuid, Arc<Node>> as Diff>::Delta>,
    wants: Option<HashMap<Owner, Vec<Query>>>,
    listings: Option<<imbl::OrdMap<Listing, Listed> as Diff>::Delta>,
    heard: Option<<imbl::OrdMap<Uuid, u64> as Diff>::Delta>,
    pending: Option<<imbl::OrdMap<Uuid, u32> as Diff>::Delta>,
    unpublished: Option<<imbl::OrdSet<Uuid> as Diff>::Delta>,
    derived: Option<<imbl::OrdMap<Uuid, Entries> as Diff>::Delta>,
    referenced: Option<<imbl::OrdMap<Uuid, Vec<Uuid>> as Diff>::Delta>,
    read: Option<<imbl::OrdMap<Uuid, (CommitId, Vec<Uuid>)> as Diff>::Delta>,
    unreadable: Option<<imbl::OrdMap<Uuid, CommitId> as Diff>::Delta>,
    reported: Option<<imbl::OrdMap<Uuid, CommitId> as Diff>::Delta>,
    placing: Option<std::collections::HashSet<Uuid>>,
    organization_settings: Option<Option<Uuid>>,
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
            heard,
            pending,
            follows: _,
            unfollowed: _,
            unpublished,
            nodes_moved: _,
            to_count: _,
            #[cfg(test)]
                copied_on_write: _,
            derived,
            referenced,
            read,
            reading: _,
            to_read_changed: _,
            retrying: _,
            unreadable,
            reported,
            placing,
            placed_recently: _,
            dropped: _,
            unlisted_held: _,
            organization_settings,
        } = new;
        let delta = GraphDelta {
            revision: changed(&old.revision, revision),
            nodes: Diff::diff(&old.nodes, nodes),
            wants: changed(&old.wants, wants),
            listings: Diff::diff(&old.listings, listings),
            heard: Diff::diff(&old.heard, heard),
            pending: Diff::diff(&old.pending, pending),
            unpublished: Diff::diff(&old.unpublished, unpublished),
            derived: Diff::diff(&old.derived, derived),
            referenced: Diff::diff(&old.referenced, referenced),
            read: Diff::diff(&old.read, read),
            unreadable: Diff::diff(&old.unreadable, unreadable),
            reported: Diff::diff(&old.reported, reported),
            placing: changed(&old.placing, placing),
            organization_settings: changed(&old.organization_settings, organization_settings),
        };
        let GraphDelta {
            revision,
            nodes,
            wants,
            listings,
            heard,
            pending,
            unpublished,
            derived,
            referenced,
            read,
            unreadable,
            reported,
            placing,
            organization_settings,
        } = &delta;
        let changed = revision.is_some()
            || nodes.is_some()
            || wants.is_some()
            || listings.is_some()
            || heard.is_some()
            || pending.is_some()
            || unpublished.is_some()
            || derived.is_some()
            || referenced.is_some()
            || read.is_some()
            || unreadable.is_some()
            || reported.is_some()
            || placing.is_some()
            || organization_settings.is_some();
        changed.then_some(delta)
    }

    fn apply(&mut self, delta: GraphDelta) {
        let GraphDelta {
            revision,
            nodes,
            wants,
            listings,
            heard,
            pending,
            unpublished,
            derived,
            referenced,
            read,
            unreadable,
            reported,
            placing,
            organization_settings,
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
        if let Some(heard) = heard {
            self.heard.apply(heard);
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
        if let Some(read) = read {
            self.read.apply(read);
        }
        if let Some(unreadable) = unreadable {
            self.unreadable.apply(unreadable);
        }
        if let Some(reported) = reported {
            self.reported.apply(reported);
        }
        if let Some(placing) = placing {
            self.placing = placing;
        }
        if let Some(organization_settings) = organization_settings {
            self.organization_settings = organization_settings;
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
            heard,
            pending,
            follows: _,
            unfollowed: _,
            unpublished,
            nodes_moved: _,
            to_count: _,
            #[cfg(test)]
                copied_on_write: _,
            derived,
            referenced,
            read,
            reading: _,
            to_read_changed: _,
            retrying: _,
            unreadable,
            reported,
            placing,
            placed_recently: _,
            dropped: _,
            unlisted_held: _,
            organization_settings,
        } = self;
        *revision == other.revision
            && *nodes == other.nodes
            && *wants == other.wants
            && *listings == other.listings
            && *heard == other.heard
            && *pending == other.pending
            && *unpublished == other.unpublished
            && *derived == other.derived
            && *referenced == other.referenced
            && *read == other.read
            && *unreadable == other.unreadable
            && *reported == other.reported
            && *placing == other.placing
            && *organization_settings == other.organization_settings
    }
}

/// How many times a head's references are read before a failure that may pass pins it
/// (`Graph::read_failed`).
const READ_ATTEMPTS: u8 = 3;

/// How many heads one `Graph::take_to_read` takes, so what is asked for later is not queued
/// behind a long tail.
const READS_TAKEN: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Retry {
    head: CommitId,
    attempts: u8,
    waiting: bool,
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
    pub by: String,
}

/// Blocks the server's answer to a listing left out while this device held each as belonging
/// in it, with the sequence the answer was read at and whether it answered a request made
/// before a change asked for the listing again.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UnlistedHeld {
    pub listing: Listing,
    pub blocks: Vec<Uuid>,
    pub sequence: u64,
    pub stale: bool,
}

impl Graph {
    fn moved(&mut self) {
        self.revision += 1;
        self.to_read_changed = true;
        crate::changes::graph();
    }

    pub fn want(&mut self, owner: Owner, queries: Vec<Query>) {
        self.to_read_changed = true;
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
            self.listings.remove(&listing);
        }
        let mut changed = self.listings.len() != before;
        for listing in desired {
            if self.listings.contains_key(&listing) {
                continue;
            }
            let members = self
                .nodes
                .values()
                .filter(|node| member(listing, node, &known!(self)) == Some(true))
                .map(|node| node.id)
                .collect();
            self.listings.insert(
                listing,
                Listed {
                    members,
                    fetch: Fetch::Wanted,
                    again: false,
                    answered: false,
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
            // The folder's own summary too: its entry is the tree followed for what joins it,
            // however empty it is.
            Query::Children(parent) => {
                into.insert(Listing::Block(parent));
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

    /// Whether `take_wanted` has anything to take: asked first, since taking changes the graph,
    /// which copies it while a `Seen` handed over shares it.
    pub fn has_listings_to_take(&self) -> bool {
        self.listings
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

    /// A listing's answer, read at the organization's `sequence`.
    pub fn listed(&mut self, listing: Listing, nodes: Vec<Node>, sequence: u64) {
        self.retry_reads();
        let Some(listed) = self.listings.get_mut(&listing) else {
            return;
        };
        let stale = std::mem::take(&mut listed.again);
        listed.answered = true;
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
                        .is_some_and(|node| member(listing, node, &known!(self)) == Some(true))
                }),
        );
        // Where this device knows a block's content (its open replica's report, or a read of its
        // head), that says whether it is in the listing, as the query answers: the server lists
        // what was published, and an edit not sealed and sent yet, such as a new child added to
        // a folder, is ahead of it until the next publish reaches the server.
        let described = match listing {
            Listing::References(block) => {
                self.referenced.contains_key(&block) || self.read.contains_key(&block)
            }
            Listing::Backrefs(_) => true,
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
                            .is_some_and(|heard| *heard > sequence)
                })
                .filter(|node| member(listing, node, &known!(self)) == Some(true))
                .map(|node| node.id),
        );
        let unlisted: Vec<Uuid> = before
            .into_iter()
            .filter(|block| !listed.members.contains(block))
            .filter(|block| {
                self.nodes
                    .get(block)
                    .is_some_and(|node| member(listing, node, &known!(self)) == Some(true))
            })
            .collect();
        if !unlisted.is_empty() {
            self.unlisted_held.push(UnlistedHeld {
                listing,
                blocks: unlisted,
                sequence,
                stale,
            });
        }
        for node in nodes {
            if self.hear(node.id, node.changed) && !self.pending.contains_key(&node.id) {
                self.insert(node, sequence);
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

    /// A new connection: every listing is read again, and every entry point followed again from
    /// where it was read to, since the server follows nothing for it yet.
    pub fn relist_all(&mut self) {
        self.heard.clear();
        // Content that could not be read offline may be read now, and a read the connection
        // dropped is read again.
        self.unreadable.clear();
        self.reading.clear();
        self.retrying.clear();
        self.to_read_changed = true;
        let entries: Vec<Uuid> = self.follows.keys().copied().collect();
        for entry in entries {
            if let Some(follow) = self.follows.get_mut(&entry) {
                follow.fetch = Fetch::Wanted;
                follow.again = false;
            }
        }
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
        // A block's references come from its content, which orders what it holds: ready once
        // read, or once its content could not be read (answered from the server's listing then).
        if let Query::References(block) | Query::Children(block) = query
            && self.nodes.get(&block).is_some_and(|node| {
                node.references.is_none()
                    && (self.wants_read(node) || self.reading.contains(&block))
            })
        {
            return false;
        }
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

    /// Whether a listing the server never answered is being read: what shows it waits for it.
    pub fn loading(&self) -> bool {
        self.listings.values().any(|listed| {
            !listed.answered && matches!(listed.fetch, Fetch::Wanted | Fetch::Fetching)
        })
    }

    /// Whether a listing the server answered before is being read again (once this device's
    /// creation or move reaches the server, on a new connection, or for a change the server
    /// pushed): what it held keeps showing meanwhile, so this waits on the server alone, and a
    /// connection that silently stopped answering holds it until it is declared lost.
    pub fn refreshing(&self) -> bool {
        self.listings.values().any(|listed| {
            listed.answered && matches!(listed.fetch, Fetch::Wanted | Fetch::Fetching)
        })
    }

    /// Whether `block` has a change made here pending, or was made here and not sent yet: the
    /// server's word on it is not news until then.
    pub fn unsettled(&self, block: Uuid) -> bool {
        self.pending.contains_key(&block) || self.unpublished.contains(&block)
    }

    /// Whether a change made here to `block` (its creation among them) waits for the peer to
    /// take it (`settle`): until then the peer knows the block as it was, or not at all.
    pub fn changing(&self, block: Uuid) -> bool {
        self.pending.contains_key(&block)
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

    /// A block's summary, read at `sequence` (its own `changed` when nothing says more).
    pub fn put(&mut self, node: Node, sequence: u64) {
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
        if self.hear(node.id, node.changed) && !self.pending.contains_key(&node.id) {
            self.insert(node, sequence);
        }
    }

    /// Puts a node read again from what this device already heard of it (keys arrived that
    /// open it); one it does not hold, has a change pending on, or heard of newer since is left
    /// as it is.
    pub fn reread(&mut self, node: Node) {
        let newest = self.heard.get(&node.id).copied().unwrap_or_default();
        if node.changed >= newest
            && self.nodes.contains_key(&node.id)
            && !self.pending.contains_key(&node.id)
        {
            let changed = node.changed;
            self.insert(node, changed);
        }
    }

    /// A block gone as of `sequence`.
    pub fn remove(&mut self, block: Uuid, sequence: u64) {
        if self.hear(block, sequence) {
            self.forget(block);
        }
    }

    fn hear(&mut self, block: Uuid, changed: u64) -> bool {
        let newest = self.heard.get(&block).copied().unwrap_or_default();
        if changed < newest {
            return false;
        }
        self.heard.insert(block, changed);
        true
    }

    /// Whether `node` belongs in what the mirror holds: it holds the block, or one of its
    /// listings would list it.
    fn holds_or_lists(&self, node: &Node) -> bool {
        self.nodes.contains_key(&node.id)
            || self.listings.contains_key(&Listing::Block(node.id))
            || node
                .parent
                .is_some_and(|parent| self.listings.contains_key(&Listing::Children(parent)))
            || (node.parent.is_some()
                && !node.held
                && self.listings.contains_key(&Listing::Deleted))
            || node
                .references
                .iter()
                .flatten()
                .any(|target| self.listings.contains_key(&Listing::Backrefs(*target)))
            || self.listings.keys().any(|listing| {
                matches!(listing, Listing::References(_))
                    && member(*listing, node, &known!(self)) == Some(true)
            })
    }

    /// Counts a node in or out of its entry point's tree (`Follow::nodes`): the first in follows
    /// it from `at`, the last out unfollows it.
    fn count_entry(&mut self, entry: Option<Uuid>, at: u64, held: bool) {
        let Some(entry) = entry else {
            return;
        };
        match held {
            true => {
                let follow = self.follows.entry(entry).or_insert_with(|| Follow {
                    cursor: at,
                    ..Follow::default()
                });
                follow.nodes += 1;
                // A tree read refused before (a grant given back since, room to follow made)
                // is tried again for what joins it.
                if matches!(follow.fetch, Fetch::Failed(_)) {
                    follow.fetch = Fetch::Wanted;
                }
            }
            false => {
                let Some(follow) = self.follows.get_mut(&entry) else {
                    return;
                };
                follow.nodes = follow.nodes.saturating_sub(1);
                if follow.nodes == 0
                    && let Some(follow) = self.follows.remove(&entry)
                    && follow.fetch != Fetch::Wanted
                {
                    self.unfollowed.push(entry);
                }
            }
        }
    }

    /// Whether `take_follows` has anything to take.
    pub fn has_follows_to_take(&self) -> bool {
        !self.unfollowed.is_empty()
            || self
                .follows
                .values()
                .any(|follow| follow.fetch == Fetch::Wanted)
    }

    /// The entry points to read (and follow) from their cursors, and those to unfollow.
    pub fn take_follows(&mut self) -> (Vec<(Uuid, u64)>, Vec<Uuid>) {
        let wanted: Vec<(Uuid, u64)> = self
            .follows
            .iter()
            .filter(|(_, follow)| follow.fetch == Fetch::Wanted)
            .map(|(entry, follow)| (*entry, follow.cursor))
            .collect();
        for (entry, _) in &wanted {
            if let Some(follow) = self.follows.get_mut(entry) {
                follow.fetch = Fetch::Fetching;
            }
        }
        (wanted, std::mem::take(&mut self.unfollowed))
    }

    /// The server says the tree of `entry` advanced to `sequence` (`tree-advanced`).
    pub fn advanced(&mut self, entry: Uuid, sequence: u64) {
        let Some(follow) = self.follows.get_mut(&entry) else {
            return;
        };
        match follow.fetch {
            Fetch::Fetching => follow.again = true,
            _ if follow.cursor < sequence => follow.fetch = Fetch::Wanted,
            _ => {}
        }
    }

    /// What changed in the tree of `entry` up to `sequence`: `nodes` changed in it, and
    /// `removed` left it. Only what the mirror holds, or a listing it holds would list, is
    /// taken. Answers the blocks it holds that left the tree, for the worker to read again
    /// wherever they went.
    pub fn followed(
        &mut self,
        entry: Uuid,
        nodes: Vec<Node>,
        removed: &[Uuid],
        sequence: u64,
    ) -> Vec<Uuid> {
        self.retry_reads();
        if let Some(follow) = self.follows.get_mut(&entry) {
            follow.cursor = follow.cursor.max(sequence);
            follow.fetch = match std::mem::take(&mut follow.again) {
                true => Fetch::Wanted,
                false => Fetch::Loaded,
            };
        }
        for node in nodes {
            // A block whose references changed, or that a block started or stopped referencing,
            // is in a changed tree: what links to it is read again.
            self.relist(Listing::Backrefs(node.id));
            if self.holds_or_lists(&node)
                && self.hear(node.id, node.changed)
                && !self.pending.contains_key(&node.id)
            {
                self.insert(node, sequence);
            }
        }
        self.prune();
        self.moved();
        self.refresh_listings();
        removed
            .iter()
            .copied()
            .filter(|block| self.nodes.contains_key(block))
            .collect()
    }

    /// A read of the tree of `entry` was refused: it is not read again until something follows
    /// it anew, and the listings holding what lies in it are read again.
    pub fn follow_failed(&mut self, entry: Uuid) {
        if let Some(follow) = self.follows.get_mut(&entry) {
            follow.fetch = Fetch::Failed(String::new());
            follow.again = false;
        }
        let holding: Vec<Listing> = self
            .listings
            .iter()
            .filter(|(_, listed)| {
                listed.members.iter().any(|block| {
                    self.nodes
                        .get(block)
                        .is_some_and(|node| node.entry == Some(entry))
                })
            })
            .map(|(listing, _)| *listing)
            .collect();
        for listing in holding {
            self.relist(listing);
        }
    }

    /// A read of the tree of `entry` failed with the connection it was asked on: it waits for
    /// the next one, which reads every followed tree again (`relist_all`).
    pub fn follow_stalled(&mut self, entry: Uuid) {
        if let Some(follow) = self.follows.get_mut(&entry) {
            follow.fetch = Fetch::Loaded;
            follow.again = false;
        }
    }

    /// A read of the tree of `entry` never got an answer: it is read again.
    pub fn unfetch_follow(&mut self, entry: Uuid) {
        if let Some(follow) = self.follows.get_mut(&entry)
            && follow.fetch == Fetch::Fetching
        {
            follow.fetch = Fetch::Wanted;
        }
    }

    /// What lies at the top, in the trash or is shared with this account may have changed, by a
    /// move, a grant or a block made at the top (`topic.roots`): those listings are read
    /// again, and the settings opened to who views a block inside (a grant too), and the blocks
    /// asked for that are not held, and every tree whose read was refused is tried again.
    pub fn roots(&mut self) {
        for listing in [
            Listing::Children(BlockParent::Root),
            Listing::Deleted,
            Listing::Shared,
        ] {
            self.relist(listing);
        }
        self.relist_open_settings();
        // A block asked for before it was made (an editor opened on a new block) answered
        // nothing; made at the top or shared since, it answers now.
        let absent: Vec<Listing> = self
            .listings
            .keys()
            .filter(|listing| matches!(listing, Listing::Block(block) if !self.nodes.contains_key(block)))
            .copied()
            .collect();
        for listing in absent {
            self.relist(listing);
        }
        let failed: Vec<Uuid> = self
            .follows
            .iter()
            .filter(|(_, follow)| matches!(follow.fetch, Fetch::Failed(_)))
            .map(|(entry, _)| *entry)
            .collect();
        for entry in failed {
            if let Some(follow) = self.follows.get_mut(&entry) {
                follow.fetch = Fetch::Wanted;
            }
        }
    }

    /// The blocks held that this account may only know exist: they name no entry point, so no
    /// tree tells of them, and a move or a grant may have opened them (`roots`).
    pub fn known_only(&self) -> Vec<Uuid> {
        self.nodes
            .values()
            .filter(|node| !node.access.can_view())
            .map(|node| node.id)
            .collect()
    }

    /// The entry points followed, for a test.
    #[cfg(any(test, feature = "testing"))]
    pub fn followed_entries(&self) -> Vec<Uuid> {
        self.follows.keys().copied().collect()
    }

    /// A block made here, kept until the drain sends it: what the server lists before then
    /// cannot hold it (`unpublished`).
    /// The organization the mirror is of: its settings block is never a root block.
    /// A root listing made before this took the settings in as it took any block at the root:
    /// they leave it, and the mirror, unless something else holds them.
    pub fn organized(&mut self, organization: Uuid) {
        let settings = be_block::organization_settings(organization);
        if self.organization_settings == Some(settings) {
            return;
        }
        self.organization_settings = Some(settings);
        if let Some(listed) = self.listings.get_mut(&Listing::Children(BlockParent::Root)) {
            listed.members.remove(&settings);
        }
        self.prune();
        self.moved();
    }

    pub fn made(&mut self, block: Uuid) {
        self.unpublished.insert(block);
    }

    pub fn change(&mut self, node: Node) {
        *self.pending.entry(node.id).or_default() += 1;
        let changed = node.changed;
        self.insert(node, changed);
    }

    /// A node to change, copied first if a `Seen` handed over shares it: the persistent map
    /// copies the path to it, and the node itself only when shared.
    /// Removes a node, answering whether it was there.
    fn remove_node(&mut self, block: Uuid) -> bool {
        let Some(removed) = self.nodes.remove(&block) else {
            return false;
        };
        self.read.remove(&block);
        self.unreadable.remove(&block);
        self.retrying.remove(&block);
        self.nodes_moved.insert(block);
        self.to_count.insert(block);
        self.count_entry(removed.entry, 0, false);
        true
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

    /// Takes the nodes added, changed or removed since the unread counts last took them
    /// (`crate::unread`), so a pass of theirs looks at those alone.
    pub(crate) fn take_to_count(&mut self) -> imbl::OrdSet<Uuid> {
        if self.to_count.is_empty() {
            return imbl::OrdSet::new();
        }
        std::mem::take(&mut self.to_count)
    }

    fn node_mut(&mut self, block: Uuid) -> Option<&mut Node> {
        if !self.nodes.contains_key(&block) {
            return None;
        }
        self.nodes_moved.insert(block);
        self.to_count.insert(block);
        let node = self.nodes.get_mut(&block)?;
        #[cfg(test)]
        if Arc::strong_count(node) > 1 {
            self.copied_on_write += 1;
        }
        Some(Arc::make_mut(node))
    }

    /// Sets how many things in `block`'s content the account has not read (`crate::unread`),
    /// when that changes it.
    pub fn set_unread(&mut self, block: Uuid, unread: u32) {
        if self
            .nodes
            .get(&block)
            .is_some_and(|node| node.unread != unread)
            && let Some(node) = self.node_mut(block)
        {
            node.unread = unread;
            self.moved();
        }
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
            // Moved even when the node stays as it was: what waited on the change (`changing`)
            // looks again.
            if !self.covers(block) {
                self.remove_node(block);
            }
            self.moved();
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
                node.references.as_ref()
                    == Some(&with_described(references.to_vec(), &node.metadata))
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
                    .is_some_and(|node| node.references.as_ref() != Some(all))
            });
        if let Some(all) = all
            && let Some(node) = self.node_mut(block)
        {
            node.references = Some(all);
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
        let Some(references) = self
            .nodes
            .get(&parent)
            .and_then(|node| node.references.as_ref())
            .map(|references| {
                references
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<Uuid>>()
            })
        else {
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
                    by: by.to_owned(),
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

    /// What `take_unlisted_held` would take now.
    #[cfg(test)]
    pub fn unlisted_held(&self) -> &[UnlistedHeld] {
        &self.unlisted_held
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

    /// An open block's replica closed: what it reported last stays what the block references
    /// at the head it reported, as if read from it (`read`). That report may hold edits its
    /// session had not sealed yet, which this device showed last; the head their seal makes is
    /// read again.
    pub fn forget_references(&mut self, block: Uuid) {
        if self.referenced.remove(&block).is_some() {
            self.to_read_changed = true;
            if let Some(node) = self.nodes.get(&block)
                && let (Some(head), Some(references)) = (node.head, node.references.clone())
            {
                self.read.insert(block, (head, references));
            }
        }
    }

    /// New keys arrived: content no key opened before may open now.
    pub fn keys_changed(&mut self) {
        if !self.unreadable.is_empty() {
            self.unreadable.clear();
            self.to_read_changed = true;
            self.moved();
        }
    }

    /// Whether a block's references may wait to be read from its head (`take_to_read`): asked
    /// first, since taking changes the graph. Any change to the graph (`moved`, which a
    /// replica's report, a listing's answer or a read sets off) makes the next pass of the
    /// worker scan every node once, O(nodes).
    pub fn has_to_read(&self) -> bool {
        self.to_read_changed
    }

    /// The blocks whose references to read from their head, with that head: every node held
    /// that this account may read, that no replica reports and no read so far covers, so what
    /// shows a block (a file tree's row, a link) knows what it holds, whether shown open or not.
    /// What is asked for comes first (its references, its children or the block itself), and
    /// at most `READS_TAKEN` are taken, so a folder just opened does not wait behind a long
    /// tail. Marked as being read until `read_references`, `read_failed` or `read_skipped`.
    pub fn take_to_read(&mut self) -> Vec<(Uuid, CommitId)> {
        self.to_read_changed = false;
        let mut found: Vec<(bool, Uuid, CommitId)> = self
            .nodes
            .values()
            .filter(|node| self.wants_read(node))
            .filter_map(|node| {
                let asked = [
                    Listing::References(node.id),
                    Listing::Children(BlockParent::Block(node.id)),
                    Listing::Block(node.id),
                ]
                .iter()
                .any(|listing| self.listings.contains_key(listing));
                Some((!asked, node.id, node.head?))
            })
            .collect();
        found.sort_by_key(|(later, _, _)| *later);
        if found.len() > READS_TAKEN {
            found.truncate(READS_TAKEN);
            self.to_read_changed = true;
        }
        for (_, block, _) in &found {
            self.reading.insert(*block);
        }
        found
            .into_iter()
            .map(|(_, block, head)| (block, head))
            .collect()
    }

    /// A read taken was not made: the block left the mirror or its head moved on meanwhile.
    pub fn read_skipped(&mut self, block: Uuid) {
        self.reading.remove(&block);
        self.to_read_changed = true;
    }

    /// The server answered (a listing, a tree's page): reads that failed for a reason that may
    /// pass are tried again.
    fn retry_reads(&mut self) {
        if self.retrying.values().any(|retry| retry.waiting) {
            for retry in self.retrying.values_mut() {
                retry.waiting = false;
            }
            self.to_read_changed = true;
        }
    }

    /// Whether `node`'s references are what its content holds now: reported by its replica,
    /// read at its head, or its metadata's with no content. Else they are from an earlier head,
    /// shown until its head is read.
    fn references_current(&self, node: &Node) -> bool {
        match node.head {
            None => true,
            Some(head) => {
                self.referenced.contains_key(&node.id)
                    || self.read.get(&node.id).is_some_and(|(at, _)| *at == head)
            }
        }
    }

    /// Whether `node`'s references at its head are still to be read: no replica reports them,
    /// nothing read them at that head, and no read of that head failed or is under way.
    fn wants_read(&self, node: &Node) -> bool {
        let Some(head) = node.head else {
            return false;
        };
        node.access.can_view()
            && !node.locked
            && !self.referenced.contains_key(&node.id)
            && !self.reading.contains(&node.id)
            && self.read.get(&node.id).is_none_or(|(at, _)| *at != head)
            && self.unreadable.get(&node.id) != Some(&head)
            && self
                .retrying
                .get(&node.id)
                .is_none_or(|retry| retry.head != head || !retry.waiting)
    }

    /// What `block`'s content references at `head`, read by the worker: its node takes them while
    /// it is at that head and no replica reports them.
    pub fn read_references(&mut self, block: Uuid, head: CommitId, references: Vec<Uuid>) {
        self.reading.remove(&block);
        self.to_read_changed = true;
        // Gone from the mirror while it was read: nothing keeps what was read.
        if !self.nodes.contains_key(&block) {
            return;
        }
        self.unreadable.remove(&block);
        self.retrying.remove(&block);
        self.read.insert(block, (head, references.clone()));
        if self.referenced.contains_key(&block) {
            return;
        }
        let at_head = self.nodes.get(&block).is_some_and(|node| {
            node.head == Some(head) && node.references.as_ref() != Some(&references)
        });
        if at_head && let Some(node) = self.node_mut(block) {
            node.references = Some(references.clone());
            self.placed(block, &references);
            // The server's listing is read again only for a block it names that is not held.
            if references
                .iter()
                .any(|reference| !self.nodes.contains_key(reference))
            {
                self.relist(Listing::References(block));
            }
            self.moved();
            self.check_dropped(block, "its content");
        }
    }

    /// `block`'s content at `head` could not be read. For a reason that lasts (`lasting`: no key
    /// opens it, it was forged or tampered with, it does not decode) or after `READ_ATTEMPTS`,
    /// it is not tried again at that head until a new connection or new keys; otherwise it is
    /// tried again once the server next answers. What shows it answers from what was known
    /// before, or the server's listings, meanwhile.
    pub fn read_failed(&mut self, block: Uuid, head: CommitId, lasting: bool) {
        self.reading.remove(&block);
        self.to_read_changed = true;
        if !self.nodes.contains_key(&block) {
            return;
        }
        let attempts = match self.retrying.get(&block) {
            Some(retry) if retry.head == head => retry.attempts + 1,
            _ => 1,
        };
        if lasting || attempts >= READ_ATTEMPTS {
            self.retrying.remove(&block);
            self.unreadable.insert(block, head);
            self.moved();
        } else {
            self.retrying.insert(
                block,
                Retry {
                    head,
                    attempts,
                    waiting: true,
                },
            );
        }
    }

    /// Puts `node` in the mirror, heard as of `at`, where it is covered, and counts it in its
    /// entry point's tree, so the mirror follows the entries of exactly what it holds.
    fn insert(&mut self, mut node: Node, at: u64) {
        self.to_read_changed = true;
        if let Some(known) = self.nodes.get(&node.id) {
            node.unread = known.unread;
        }
        if let Some(derived) = self.derived.get(&node.id) {
            node.metadata.derived.clone_from(derived);
        }
        if let Some(head) = self.reported.get(&node.id) {
            node.head = Some(*head);
        }
        if let Some(references) = self.referenced.get(&node.id) {
            node.references = Some(with_described(references.clone(), &node.metadata));
        } else if node.references.is_none() {
            // No summary names them: what was read at this head, or for a block with no
            // content yet, its metadata's; at a head not read yet, what was known before, until
            // the read lands (`wants_read`).
            let known = || {
                self.nodes
                    .get(&node.id)
                    .and_then(|known| known.references.clone())
            };
            node.references = match node.head {
                _ if !node.access.can_view() || node.locked => None,
                Some(head) => self
                    .read
                    .get(&node.id)
                    .filter(|(at, _)| *at == head)
                    .map(|(_, references)| references.clone())
                    .or_else(known),
                None => Some(node.metadata.references()),
            };
        }
        let changed_references = self
            .nodes
            .get(&node.id)
            .is_some_and(|known| known.references != node.references);
        if changed_references {
            self.relist(Listing::References(node.id));
        }
        if let Some(references) = node.references.clone() {
            self.placed(node.id, &references);
        }
        // Only the listings whose membership changes are written: a persistent map copies what
        // it writes while a `Seen` handed over shares it.
        let moved: Vec<(Listing, bool)> = self
            .listings
            .iter()
            .filter_map(|(listing, listed)| {
                let member = member(*listing, &node, &known!(self))?;
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
            false => "its content",
        };
        let id = node.id;
        if self.nodes.get(&node.id).map(|known| &**known) != Some(&node) {
            let before = self.nodes.get(&node.id).map(|known| known.entry);
            let entry = node.entry;
            self.nodes_moved.insert(node.id);
            self.to_count.insert(node.id);
            self.nodes.insert(node.id, Arc::new(node));
            if before != Some(entry) {
                if let Some(before) = before {
                    self.count_entry(before, at, false);
                }
                self.count_entry(entry, at, true);
            }
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
            Query::Roots => {
                let mut roots = self.with_parent(BlockParent::Root);
                roots.retain(|node| self.organization_settings != Some(node.id));
                roots
            }
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
            // In the content's order; while it cannot be read, what the server lists.
            Query::References(block) => match self.nodes.get(&block) {
                Some(node) if node.references.is_some() => {
                    return node
                        .contents()
                        .iter()
                        .filter_map(|reference| {
                            self.nodes.get(reference).map(|node| Node::clone(node))
                        })
                        .collect();
                }
                Some(_) => self.listed_nodes(Listing::References(block)),
                None => return Vec::new(),
            },
            // Those whose content is read at their head and references it, and those the server
            // lists whose content is not read there.
            Query::Backrefs(block) => {
                let listed = self.listings.get(&Listing::Backrefs(block));
                let known = known!(self);
                self.nodes
                    .values()
                    .filter(
                        |node| match member(Listing::Backrefs(block), node, &known) {
                            Some(refers) => refers,
                            None => listed.is_some_and(|listed| listed.members.contains(&node.id)),
                        },
                    )
                    .map(|node| Node::clone(node))
                    .collect()
            }
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

    fn listed_nodes(&self, listing: Listing) -> Vec<Node> {
        self.listings
            .get(&listing)
            .into_iter()
            .flat_map(|listed| listed.members.iter())
            .filter_map(|block| self.nodes.get(block))
            .map(|node| Node::clone(node))
            .collect()
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

    /// Whether `node`'s parent holds it: its parent's content references it, with each parent's
    /// references made a set once in `sets`, so asking of every child of a folder of many is not
    /// quadratic; or, while the parent's content is not read, as the server says (`held`).
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
                // From an earlier head, what the server says holds too: a child another device
                // added since is not taken for deleted until the new head is read.
                Some(parent) if parent.access.can_view() => match &parent.references {
                    _ if parent.unfiled() => true,
                    Some(references) => {
                        sets.entry(parent.id)
                            .or_insert_with(|| references.iter().copied().collect())
                            .contains(&node.id)
                            || (node.held && !self.references_current(parent))
                    }
                    None => node.held,
                },
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

/// Whether `node` belongs in `listing` as the server lists it, where this device can tell. The
/// organization's settings lie at the root but are never among its children, and a block counts
/// among another's backrefs by its references only while they are current (`Known::current`).
fn member(listing: Listing, node: &Node, known: &Known) -> Option<bool> {
    match listing {
        Listing::Children(BlockParent::Root) if known.organization_settings == Some(node.id) => {
            Some(false)
        }
        Listing::Children(parent) => Some(node.parent == Some(parent)),
        Listing::Deleted => Some(node.parent.is_some() && !node.held),
        Listing::Block(block) => Some(block == node.id),
        Listing::Backrefs(block) => known.current(node).then(|| node.refers_to(block)).flatten(),
        Listing::References(block) => known
            .nodes
            .get(&block)
            .and_then(|referrer| referrer.refers_to(node.id)),
        Listing::Shared | Listing::OpenSettings(_) => None,
    }
}

#[cfg(test)]
mod tests;
