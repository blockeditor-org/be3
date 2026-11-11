//! Full-text search (plans/search.md): each root block's search index, which the devices that
//! edit its tree write, and the searches the host and its apps ask for.
//!
//! **Indexing.** A head this device's session moved (`Host::status` while it owns the session)
//! queues its block (`Searching::dirty`). Each round of the worker's loop (`step`) takes the
//! queued blocks whose root it may edit, reads their documents' text with `search-text` and
//! the block's name from the mirror, and hands each
//! index one batch in a task (`write`), which applies it to the index's local head and seals
//! the commit into the outbox, so it is saved offline-first and published by the next drain;
//! two devices' batches meet as a diverged block and merge by parts. A root with no index gets
//! one, made under it and named by its `SEARCH_INDEX` metadata.
//!
//! **Searching.** `want` names the searches an owner shows. A search runs in a task over the
//! index of every root the account views in the open organization (`run`), reading only the
//! parts it needs; the loop then cuts each shown hit's snippet from its document's text
//! (`search-text` with its key), kept per document and head, and answers. A search runs again when an index it read moves.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    hash::{Hash as _, Hasher as _},
    ops::Range,
    sync::{Arc, PoisonError},
};

use be_block::{
    BlockContent, BlockMetadata, SEARCH_INDEX,
    be_block_model::BlockRef,
    search_index::{AsyncRead, SearchIndexContent},
};
use be_block_client::{Peer, Write};
use be_block_commit::CommitId;
use be_block_graph::{BlockParent, EDIT_KINDS, SEARCH_INDEX_TYPE, SETTINGS_TYPE};
use be_block_store::{ObjectStore, PartRef};
use be_lib_search::{Batch, DocId, Put, Query, Text};
use be_wasm_threads::Mutex;
use uuid::Uuid;

use crate::{
    Graph, Owner,
    graph::Query as GraphQuery,
    replicas::Replicas,
    worker::{Shared, Store, Tasks},
};

/// Hits a search reads from each index, the best first.
const HITS: usize = 50;

/// Hits a search answers with a snippet.
const SHOWN: usize = 20;

/// The most of a document's text the index takes, in bytes: a larger one is indexed by its
/// start.
pub(crate) const MAX_TEXT: usize = 1 << 20;

/// Texts kept for snippets, by document and head, before they are all let go.
const TEXTS: usize = 256;

/// A search an owner shows: words, `"quoted phrases"`, the last word a prefix
/// (`be_lib_search::Query::parse`), within a block's tree or everything the account views.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SearchAsk {
    pub text: String,
    pub within: Option<Uuid>,
}

/// A document a search found: its block, the document's key within it (empty for a block that
/// is one document), and where it matched.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchHit {
    pub block: Uuid,
    pub key: String,
    /// The stretch of the text around the first match, or its start when only the name
    /// matched.
    pub snippet: String,
    /// The matched words, as byte ranges of `snippet`.
    pub matches: Vec<Range<usize>>,
}

/// A search's answer, the best hit first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Searched {
    pub hits: Vec<SearchHit>,
    /// Every queued edit this device made is indexed.
    pub complete: bool,
}

#[derive(Clone, Debug)]
struct Found {
    block: Uuid,
    key: String,
    commit: be_lib_search::Commit,
}

struct Answer {
    /// The heads of the indexes it read (`heads`).
    read: u64,
    found: Vec<Found>,
    shown: Option<Searched>,
    /// The graph's revision when it was shown, if some hit was left out for want of its block
    /// in the mirror: it is shown again once the graph moves.
    missing: Option<u64>,
}

/// What the worker keeps for search, in `Shared::search`.
#[derive(Default)]
pub struct Searching {
    /// Blocks whose text their root's index has yet to take.
    dirty: BTreeSet<Uuid>,
    /// Indexes a batch is being written to.
    writing: HashSet<Uuid>,
    /// Roots whose index this device made, until their metadata names it.
    creating: HashMap<Uuid, Uuid>,
    /// Indexes made and not yet named in their root's metadata (`name_indexes`).
    made: Vec<(Uuid, Uuid)>,
    /// Roots whose made index waits for the root's open replica to seal its edits, which
    /// may be long: the host is not busy for it.
    deferred: HashSet<Uuid>,
    /// Failed attempts to name each root's index.
    tries: HashMap<Uuid, u32>,
    /// Blocks whose root's index is being made or named (`creating`).
    waiting: BTreeSet<Uuid>,
    /// Blocks whose index has no head on this device yet (made by another device, not
    /// arrived): queued again once it has one, not every round.
    parked: Vec<(Uuid, Vec<Uuid>)>,
    /// Indexes this device named, by root.
    named: HashMap<Uuid, Uuid>,
    /// The graph revision `unindexed_roots` last looked at.
    scanned: Option<u64>,
    wanted: HashMap<Owner, Vec<SearchAsk>>,
    answers: HashMap<SearchAsk, Answer>,
    running: HashSet<SearchAsk>,
    /// Documents' text for snippets, by block, key and head.
    texts: HashMap<(Uuid, String, Option<CommitId>), Option<String>>,
    /// Counts answers that changed, for askers to read again.
    pub revision: u64,
    /// Set by a test that writes as a device without an indexer.
    pub off: bool,
}

impl Searching {
    /// Queues a block whose head this device's session moved.
    pub(crate) fn sealed(&mut self, block: Uuid) {
        self.dirty.insert(block);
    }

    /// Whether blocks wait to be indexed, an index is being made or named, a batch is being
    /// written or a wanted search is not answered yet. The index counts in the size of the
    /// tree it is under, so a painting taken before it is named would change.
    pub(crate) fn busy(&self, graph_revision: u64) -> bool {
        // A graph the loop has not looked at yet may hold a workspace that wants its index.
        (!self.off
            && (!self.dirty.is_empty()
                || self
                    .creating
                    .keys()
                    .any(|root| !self.deferred.contains(root))
                || self.scanned != Some(graph_revision)))
            || !self.writing.is_empty()
            || !self.running.is_empty()
            || self.wanted().any(|asked| {
                self.answers
                    .get(asked)
                    .is_none_or(|answer| answer.shown.is_none())
            })
    }

    fn wanted(&self) -> impl Iterator<Item = &SearchAsk> {
        self.wanted.values().flatten()
    }

    pub(crate) fn want(&mut self, owner: Owner, asked: Vec<SearchAsk>) -> bool {
        // Nothing asked, and nothing was: not a change, and callers say so every frame.
        if self.wanted.get(&owner) == Some(&asked)
            || (asked.is_empty() && !self.wanted.contains_key(&owner))
        {
            return false;
        }
        match asked.is_empty() {
            true => self.wanted.remove(&owner),
            false => self.wanted.insert(owner, asked),
        };
        let wanted: HashSet<SearchAsk> = self.wanted().cloned().collect();
        self.answers.retain(|asked, _| wanted.contains(asked));
        true
    }

    pub(crate) fn answer(&self, asked: &SearchAsk) -> Option<Searched> {
        let mut shown = self.answers.get(asked)?.shown.clone()?;
        shown.complete = self.indexed();
        Some(shown)
    }

    /// Nothing this device could index is queued, parked, being made or being written.
    pub(crate) fn indexed(&self) -> bool {
        self.dirty.is_empty()
            && self.writing.is_empty()
            && self.waiting.is_empty()
            && self.parked.is_empty()
            && self.creating.is_empty()
    }

    /// An index that could not be made: its root's blocks that waited for it are looked at
    /// again, and find no index to wait for.
    pub(crate) fn make_failed(&mut self, root: Uuid) {
        self.creating.remove(&root);
        self.deferred.remove(&root);
        self.tries.remove(&root);
        self.made.retain(|(made, _)| *made != root);
        let waiting = std::mem::take(&mut self.waiting);
        self.dirty.extend(waiting);
    }

    /// Blocks parked for an index queue again once it has a head (`arrived`) or no root
    /// names it any more (`named`): the second finds them no index, so they drop out.
    pub(crate) fn release_parked(
        &mut self,
        arrived: impl Fn(Uuid) -> bool,
        named: impl Fn(Uuid) -> bool,
    ) {
        for (index, blocks) in std::mem::take(&mut self.parked) {
            match arrived(index) || !named(index) {
                true => self.dirty.extend(blocks),
                false => self.parked.push((index, blocks)),
            }
        }
    }

    /// Queues a block to wait for its root's index, which is being made (a test's way into
    /// `index`'s bookkeeping).
    #[cfg(test)]
    pub(crate) fn wait_for_index(&mut self, root: Uuid, index: Uuid, block: Uuid) {
        self.creating.insert(root, index);
        self.waiting.insert(block);
    }

    /// Marks the graph as looked at, and a root's made index as waiting for its replica.
    #[cfg(test)]
    pub(crate) fn defer(&mut self, root: Uuid, revision: u64) {
        self.scanned = Some(revision);
        self.deferred.insert(root);
    }

    /// Drops the blocks waiting for an index (a test looks at `busy` alone).
    #[cfg(test)]
    pub(crate) fn take_waiting_for_test(&mut self) {
        self.waiting.clear();
    }

    /// Takes the queued blocks (a test's way to see what `index` would look at).
    #[cfg(test)]
    pub(crate) fn take_queued(&mut self) -> Vec<Uuid> {
        std::mem::take(&mut self.dirty).into_iter().collect()
    }

    /// Parks blocks for an index that has no head on this device yet.
    pub(crate) fn park(&mut self, index: Uuid, blocks: Vec<Uuid>) {
        self.parked.push((index, blocks));
    }
}

enum Place {
    /// The mirror does not reach its root yet.
    Unknown,
    /// Not indexed: detached, or a block its owner holds by metadata.
    Skip,
    Root(Uuid),
}

/// The root whose index holds `block`'s text.
fn place(graph: &Graph, block: Uuid) -> Place {
    let mut current = block;
    for _ in 0..=graph.nodes().count() {
        let Some(node) = graph.get(current) else {
            return Place::Unknown;
        };
        let held_by_metadata = node.content_type == SEARCH_INDEX_TYPE
            || node.content_type == SETTINGS_TYPE
            || EDIT_KINDS.contains(&node.content_type);
        match node.parent {
            Some(BlockParent::Block(_)) if held_by_metadata => return Place::Skip,
            Some(BlockParent::Block(parent)) => current = parent,
            Some(BlockParent::Root) if current == block => return Place::Skip,
            Some(BlockParent::Root) => return Place::Root(current),
            Some(BlockParent::Detached) => return Place::Skip,
            None => return Place::Unknown,
        }
    }
    Place::Skip
}

/// A root's index: as its metadata names it, else the one this device named, which a listing
/// from before the define may hide for a while.
fn index_of(graph: &Graph, named: &HashMap<Uuid, Uuid>, root: Uuid) -> Option<Uuid> {
    graph
        .get(root)?
        .metadata
        .get(&SEARCH_INDEX)
        .map(|BlockRef(index)| index)
        .or_else(|| named.get(&root).copied())
}

/// One round of the worker's loop: batches for the indexes, searches to run, snippets to cut.
/// Answers whether anything changed.
pub(crate) async fn step(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    tasks: &Tasks,
) -> bool {
    name_indexes(peer, shared, replicas, tasks).await;
    let mut moved = index(peer, shared, replicas, tasks).await;
    moved |= search(peer, shared, tasks);
    moved |= show(peer, shared, replicas).await;
    moved
}

async fn index(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    tasks: &Tasks,
) -> bool {
    let (batches, roots) = {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        if held.search.off {
            return false;
        }
        let roots = unindexed_roots(&mut held);
        let state = &mut *held;
        let (graph, named) = (&state.graph, &state.search.named);
        let roots_naming: HashSet<Uuid> = graph
            .nodes()
            .filter(|node| node.parent == Some(BlockParent::Root))
            .filter_map(|node| index_of(graph, named, node.id))
            .collect();
        state.search.release_parked(
            |index| {
                peer.local_head(index).is_some()
                    || graph.get(index).is_some_and(|node| node.head.is_some())
            },
            |index| roots_naming.contains(&index),
        );
        if held.search.dirty.is_empty() {
            drop(held);
            make_indexes(peer, shared, tasks, roots);
            return false;
        }
        let dirty = std::mem::take(&mut held.search.dirty);
        let mut batches: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        let mut kept = BTreeSet::new();
        for block in dirty {
            let root = match place(&held.graph, block) {
                Place::Unknown if held.graph.fetching() => {
                    kept.insert(block);
                    continue;
                }
                Place::Unknown | Place::Skip => continue,
                Place::Root(root) => root,
            };
            if !held
                .graph
                .get(root)
                .is_some_and(|root| root.access.can_edit())
            {
                continue;
            }
            match index_of(&held.graph, &held.search.named, root) {
                Some(index) if held.search.writing.contains(&index) => {
                    kept.insert(block);
                }
                Some(index) => {
                    held.search.creating.remove(&root);
                    batches.entry(index).or_default().push(block);
                }
                // Its index is being made or named: queued again once it is.
                None if held.search.creating.contains_key(&root) => {
                    held.search.waiting.insert(block);
                }
                None => {}
            }
        }
        // One block a round: asking its type for text holds up the loop, so edits and opens
        // queued meanwhile go first and the rest wait for the next round.
        let mut batches = batches.into_iter();
        let first = batches.next().map(|(index, mut blocks)| {
            kept.extend(blocks.drain(1..));
            (index, blocks)
        });
        kept.extend(batches.flat_map(|(_, blocks)| blocks));
        held.search.dirty = kept;
        held.search
            .writing
            .extend(first.iter().map(|(index, _)| *index));
        (first, roots)
    };
    make_indexes(peer, shared, tasks, roots);
    // The batch's task wakes the loop for the next block when it finishes.
    let moved = batches.is_some();
    if let Some((index, blocks)) = batches {
        let mut batch = Batch::default();
        for block in &blocks {
            batch
                .put
                .extend(puts_of(peer, shared, replicas, *block).await);
        }
        let (peer, shared, tasks_held) = (Arc::clone(peer), Arc::clone(shared), tasks.clone());
        tasks.spawn(async move {
            let written = write(&peer, index, batch).await;
            // Sent before the batch stops counting as busy, so the host is never idle with the
            // sealed commit unsent.
            tasks_held.sync(&peer, &shared);
            {
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                held.search.writing.remove(&index);
                match written {
                    // Every answer is read again, whichever index it read.
                    Ok(true) => held.search.answers.clear(),
                    // Not on this device yet: made by a command still queued.
                    Ok(false) => held.search.park(index, blocks),
                    Err(error) => held.error = Some(format!("Indexing for search failed: {error}")),
                }
            }
            be_host_base::host::wake();
        });
    }
    moved
}

/// The workspaces in the mirror this device may edit that name no index yet, each marked as
/// being made: every workspace gets its index as soon as a device that edits it sees it, so
/// naming it never lands in the middle of someone's work. Looked for again only when the
/// graph moved.
fn unindexed_roots(held: &mut Shared) -> Vec<(Uuid, Uuid)> {
    let revision = held.graph.revision;
    if held.search.scanned == Some(revision) {
        return Vec::new();
    }
    held.search.scanned = Some(revision);
    let roots: Vec<Uuid> = held
        .graph
        .nodes()
        .filter(|node| {
            node.parent == Some(BlockParent::Root)
                && node.content_type == be_block::WORKSPACE_TYPE
                && node.access.can_edit()
                && !node.locked
                && !node.metadata.contains(&SEARCH_INDEX)
        })
        .map(|node| node.id)
        .filter(|root| {
            !held.search.creating.contains_key(root) && !held.search.named.contains_key(root)
        })
        .collect();
    roots
        .into_iter()
        .map(|root| {
            let index = Uuid::now_v7();
            held.search.creating.insert(root, index);
            (root, index)
        })
        .collect()
}

fn make_indexes(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: &Tasks,
    roots: Vec<(Uuid, Uuid)>,
) {
    for (root, index) in roots {
        let (peer, shared, tasks_held) = (Arc::clone(peer), Arc::clone(shared), tasks.clone());
        tasks.spawn(async move {
            let made = make_index(&peer, root, index).await;
            tasks_held.sync(&peer, &shared);
            {
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                match made {
                    Ok(()) => held.search.made.push((root, index)),
                    Err(error) => {
                        held.search.make_failed(root);
                        held.error = Some(format!("Making a search index failed: {error}"));
                    }
                }
            }
            be_host_base::host::wake();
        });
    }
}

/// What a block's documents hold at its head: its name, and each document its type answers
/// `search-text` with. A type that answers nothing is found by its name alone.
async fn puts_of(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    block: Uuid,
) -> Vec<Put> {
    let Some((content_type, name, head)) = ({
        let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.graph.get(block).map(|node| {
            (
                node.content_type,
                node.metadata.name().unwrap_or_default(),
                node.head,
            )
        })
    }) else {
        return Vec::new();
    };
    let Some(head) = peer.local_head(block).or(head) else {
        return Vec::new();
    };
    let Ok(commit) = peer.load_commit(head).await else {
        return Vec::new();
    };
    let at = be_lib_search::Commit {
        time: commit.time,
        id: *head.hash().as_bytes(),
    };
    // Only a module already running is asked: starting one would hold up the worker's loop.
    let mut documents = match crate::kind_of(content_type) {
        Some(_) => documents_of(peer, shared, replicas, content_type, &commit, None).await,
        None => Vec::new(),
    };
    if !documents.iter().any(|document| document.key.is_empty()) {
        documents.push(be_sdk_type_api::SearchDocument {
            key: String::new(),
            source: None,
            name: None,
            body: Some(String::new()),
        });
    }
    documents
        .into_iter()
        .map(|document| Put {
            doc: DocId {
                block: block.as_bytes().to_vec(),
                key: document.key.clone(),
            },
            commit: at,
            source: None,
            text: Some(Text {
                name: match document.key.is_empty() {
                    true => name.clone(),
                    false => document.name.unwrap_or_default(),
                },
                body: capped(document.body.unwrap_or_default()),
            }),
        })
        .collect()
}

/// `text` cut at `MAX_TEXT` bytes, on a character.
pub(crate) fn capped(mut text: String) -> String {
    if text.len() > MAX_TEXT {
        let mut end = MAX_TEXT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

/// A commit's searchable documents, as its type answers `search-text` (only `keys`, when
/// some are named); none when the type answers nothing or could not be asked.
async fn documents_of(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    content_type: Uuid,
    commit: &be_block_commit::Commit,
    keys: Option<Vec<String>>,
) -> Vec<be_sdk_type_api::SearchDocument> {
    replicas
        .search_text((peer, shared), content_type, &commit.manifest, keys, &[])
        .await
        .map(|searched| searched.documents)
        .unwrap_or_default()
}

/// A document's text at a block's head, for a snippet.
async fn text_of(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    block: Uuid,
    content_type: Uuid,
    key: &str,
) -> Option<String> {
    let head = peer.newest_head(block).await.ok().flatten()?;
    let commit = peer.load_commit(head).await.ok()?;
    let keys = Some(vec![key.to_owned()]);
    documents_of(peer, shared, replicas, content_type, &commit, keys)
        .await
        .into_iter()
        .find(|document| document.key == key)?
        .body
}

/// Makes `root`'s index, empty: in a task, beside the worker's loop, offline-first like any
/// create; `name_indexes` then names it in the root's metadata.
async fn make_index(peer: &Peer<Store>, root: Uuid, index: Uuid) -> Result<(), String> {
    peer.create_block(
        index,
        SEARCH_INDEX_TYPE,
        BlockParent::Block(root),
        &BlockMetadata::default(),
        false,
    )
    .await
    .map_err(|error| error.to_string())?;
    let empty = SearchIndexContent::default();
    let write = Write::encoded(SEARCH_INDEX_TYPE, empty.parts(), None)
        .described(Vec::new(), be_block::derived_metadata(&empty));
    peer.seal(index, write)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Names a made index in its root's metadata, as a define command does: in the loop, so the
/// root's open replica is told its head moved here rather than meeting a stranger's commit.
async fn name_indexes(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    tasks: &Tasks,
) {
    // A root whose open replica holds edits not sealed yet waits until they are: a define
    // under a moving session loses, and trying again each round would starve the loop.
    let made: Vec<(Uuid, Uuid)> = {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        // A root left, or one this account may no longer edit, has no index to name.
        let (named, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut held.search.made)
            .into_iter()
            .partition(|(root, _)| {
                held.graph
                    .get(*root)
                    .is_some_and(|node| node.access.can_edit() && !node.locked)
            });
        for (root, _) in gone {
            held.search.make_failed(root);
        }
        let (ready, waiting): (Vec<_>, Vec<_>) = named.into_iter().partition(|(root, _)| {
            replicas
                .status(*root)
                .is_none_or(|status| status.clean && !status.owes_seal)
        });
        held.search.deferred = waiting.iter().map(|(root, _)| *root).collect();
        held.search.made = waiting;
        ready
    };
    for (root, index) in made {
        let defined = peer
            .define(root, |metadata| {
                metadata.insert(&SEARCH_INDEX, &BlockRef(index));
            })
            .await;
        if let Ok(summary) = &defined
            && let Some(head) = peer.local_head(root).or(summary.head)
        {
            replicas.moved_here(root, head);
        }
        let sized = defined
            .as_ref()
            .ok()
            .map(|summary| (summary.size, summary.tree_size));
        // The define is a commit in the outbox: sent now, not at some later drain, so the
        // host never sits idle with it unsaved.
        if defined.is_ok() {
            tasks.sync(peer, shared);
        }
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        match defined {
            // The mirror takes the name at once, and settles: it is defined already.
            Ok(_) => {
                held.search.creating.remove(&root);
                held.search.named.insert(root, index);
                let waiting = std::mem::take(&mut held.search.waiting);
                held.search.dirty.extend(waiting);
                held.graph.update(root, |node| {
                    node.metadata.insert(&SEARCH_INDEX, &BlockRef(index));
                    // The define changed the root's bytes: shown as the server keeps them.
                    if let Some((size, tree_size)) = sized {
                        node.size = size;
                        node.tree_size = tree_size;
                    }
                });
                held.graph.settle(root);
            }
            // Most often a session that kept moving the root: named again next round, a few
            // times; a refusal, or too many, leaves the root without an index.
            Err(error) => {
                let tries = held.search.tries.entry(root).or_default();
                *tries += 1;
                match matches!(error, be_block_client::ClientError::Refused(..)) || *tries >= 8 {
                    true => {
                        held.search.make_failed(root);
                        held.error = Some(format!("Naming a search index failed: {error}"));
                    }
                    false => held.search.made.push((root, index)),
                }
            }
        }
    }
}

/// Reads a range of a saved part through a peer.
struct PeerParts<'a, S: ObjectStore>(&'a Peer<S>);

impl<S: ObjectStore> AsyncRead for PeerParts<'_, S> {
    async fn read(&self, part: PartRef, range: Range<u64>) -> Result<Vec<u8>, String> {
        self.0
            .read_part(part, range.start, range.end - range.start)
            .await
            .map_err(|error| error.to_string())
    }
}

/// An index's newest head this device knows: its own unsent one, else the server's, else the
/// one it last saw.
async fn head_of<S: ObjectStore>(peer: &Peer<S>, index: Uuid) -> Option<CommitId> {
    match peer.newest_head(index).await {
        Ok(Some(head)) => Some(head),
        Ok(None) | Err(_) => peer.local_head(index),
    }
}

async fn load<S: ObjectStore>(
    peer: &Peer<S>,
    head: CommitId,
) -> Result<SearchIndexContent, String> {
    let commit = peer
        .load_commit(head)
        .await
        .map_err(|error| error.to_string())?;
    let stored = peer
        .load_first_part(&commit.manifest)
        .await
        .map_err(|error| error.to_string())?;
    SearchIndexContent::from_stored(&stored).map_err(|error| error.to_string())
}

/// Applies a batch to the index's newest head and seals the commit into the outbox. False when
/// the index has no head on this device yet.
pub async fn write<S: ObjectStore>(
    peer: &Peer<S>,
    index: Uuid,
    mut batch: Batch,
) -> Result<bool, String> {
    let Some(head) = head_of(peer, index).await else {
        return Ok(false);
    };
    let mut content = load(peer, head).await?;
    outdate_ancestors(peer, &content, &mut batch).await?;
    content
        .apply(&PeerParts(peer), Uuid::new_v4().into_bytes(), batch)
        .await
        .map_err(|error| error.to_string())?;
    let write = Write::encoded(SEARCH_INDEX_TYPE, content.parts(), Some(head))
        .described(Vec::new(), be_block::derived_metadata(&content));
    peer.seal(index, write)
        .await
        .map_err(|error| error.to_string())?;
    Ok(true)
}

/// A put wins over the entry the index holds for its document only when its commit is the
/// later: by time, then by id. Commits made in the same millisecond tie on time and the id
/// is arbitrary, so a put that descends from the held commit takes the time after it: a
/// descendant is always the newer.
async fn outdate_ancestors<S: ObjectStore>(
    peer: &Peer<S>,
    content: &SearchIndexContent,
    batch: &mut Batch,
) -> Result<(), String> {
    for put in &mut batch.put {
        let held = content
            .entries_of(&PeerParts(peer), &put.doc.block)
            .await
            .map_err(|error| error.to_string())?;
        let Some(old) = held.iter().find(|entry| entry.doc == put.doc) else {
            continue;
        };
        if old.commit < put.commit || old.commit.id == put.commit.id {
            continue;
        }
        let (old_id, new_id) = (
            CommitId::from_hash(be_block_store::Hash::from_bytes(old.commit.id)),
            CommitId::from_hash(be_block_store::Hash::from_bytes(put.commit.id)),
        );
        if peer
            .descends(new_id, old_id)
            .await
            .map_err(|error| error.to_string())?
        {
            put.commit.time = old.commit.time.saturating_add(1);
        }
    }
    Ok(())
}

/// The best hits of `query` in one index, at its newest head.
pub async fn search_index<S: ObjectStore>(
    peer: &Peer<S>,
    index: Uuid,
    query: &Query,
    limit: usize,
) -> Result<Vec<be_lib_search::Hit>, String> {
    let Some(head) = head_of(peer, index).await else {
        return Ok(Vec::new());
    };
    load(peer, head)
        .await?
        .search(&PeerParts(peer), query, limit)
        .await
        .map_err(|error| error.to_string())
}

/// The indexes a search reads: those of the roots the account views, or of the root
/// `within` lies in.
fn indexes(graph: &Graph, named: &HashMap<Uuid, Uuid>, within: Option<Uuid>) -> Vec<Uuid> {
    let roots: Vec<Uuid> = match within {
        Some(block) => match place(graph, block) {
            Place::Root(root) => vec![root],
            Place::Unknown | Place::Skip => match graph.get(block).and_then(|node| node.parent) {
                Some(BlockParent::Root) => vec![block],
                _ => Vec::new(),
            },
        },
        None => graph
            .nodes()
            .filter(|node| node.parent == Some(BlockParent::Root) && node.access.can_view())
            .map(|node| node.id)
            .collect(),
    };
    let mut indexes: Vec<Uuid> = roots
        .into_iter()
        .filter_map(|root| index_of(graph, named, root))
        .collect();
    indexes.sort_unstable();
    indexes
}

/// What the indexes' heads are, as far as this device knows: an answer read at other heads is
/// stale.
fn heads(peer: &Peer<Store>, graph: &Graph, indexes: &[Uuid]) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    for index in indexes {
        index.hash(&mut hasher);
        peer.local_head(*index).hash(&mut hasher);
        graph
            .get(*index)
            .and_then(|node| node.head)
            .hash(&mut hasher);
    }
    hasher.finish()
}

/// Starts the wanted searches whose answer is missing or read older heads.
fn search(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks) -> bool {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let graph = held.graph.revision;
    for answer in held.search.answers.values_mut() {
        if answer.missing.is_some_and(|at| at != graph) {
            answer.shown = None;
            answer.missing = None;
        }
    }
    let wanted: Vec<SearchAsk> = held.search.wanted().cloned().collect();
    let mut followed = Vec::new();
    let mut started = false;
    for asked in wanted {
        let indexes = indexes(&held.graph, &held.search.named, asked.within);
        followed.extend(indexes.iter().map(|index| GraphQuery::Block(*index)));
        let read = heads(peer, &held.graph, &indexes);
        if held.search.running.contains(&asked)
            || held
                .search
                .answers
                .get(&asked)
                .is_some_and(|answer| answer.read == read)
        {
            continue;
        }
        held.search.running.insert(asked.clone());
        started = true;
        let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
        tasks.spawn(async move {
            let found = run(&peer, &asked, &indexes).await;
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            held.search.running.remove(&asked);
            if held.search.wanted().any(|wanted| *wanted == asked) {
                held.search.answers.insert(
                    asked,
                    Answer {
                        read,
                        found,
                        shown: None,
                        missing: None,
                    },
                );
            }
        });
    }
    let hits: BTreeSet<Uuid> = held
        .search
        .answers
        .values()
        .flat_map(|answer| answer.found.iter().take(SHOWN).map(|found| found.block))
        .collect();
    followed.extend(hits.into_iter().map(GraphQuery::Block));
    let mut seen = HashSet::new();
    followed.retain(|query| seen.insert(*query));
    if held.graph.wanted(Owner::Search) != followed.as_slice() {
        held.graph.want(Owner::Search, followed);
    }
    started
}

/// Every index's best hits, one per document: where two indexes hold a document, the later
/// commit.
async fn run(peer: &Peer<Store>, asked: &SearchAsk, indexes: &[Uuid]) -> Vec<Found> {
    let query = Query::parse(&asked.text);
    let mut best: HashMap<DocId, (f64, be_lib_search::Commit)> = HashMap::new();
    for index in indexes {
        let Ok(hits) = search_index(peer, *index, &query, HITS).await else {
            continue;
        };
        for hit in hits {
            let kept = best.entry(hit.doc).or_insert((hit.score, hit.commit));
            if hit.commit > kept.1 {
                *kept = (hit.score, hit.commit);
            }
        }
    }
    let mut found: Vec<(f64, Found)> = best
        .into_iter()
        .filter_map(|(doc, (score, commit))| {
            Some((
                score,
                Found {
                    block: Uuid::from_slice(&doc.block).ok()?,
                    key: doc.key,
                    commit,
                },
            ))
        })
        .collect();
    found.sort_by(|(a, first), (b, second)| {
        b.total_cmp(a)
            .then(second.commit.cmp(&first.commit))
            .then(first.block.cmp(&second.block))
    });
    found.into_iter().map(|(_, found)| found).collect()
}

/// Cuts the snippets of every answer not shown yet, from its hits' text, and drops hits the
/// account may no longer view or that left the tree asked for.
async fn show(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
) -> bool {
    let unshown: Vec<(SearchAsk, Vec<Found>)> = {
        let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.search
            .answers
            .iter()
            .filter(|(_, answer)| answer.shown.is_none())
            .map(|(asked, answer)| (asked.clone(), answer.found.clone()))
            .collect()
    };
    if unshown.is_empty() {
        return false;
    }
    for (asked, found) in unshown {
        let query = Query::parse(&asked.text);
        let mut hits = Vec::new();
        let mut missing = false;
        for found in found {
            let (content_type, head) = {
                let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(node) = held.graph.get(found.block) else {
                    missing = true;
                    continue;
                };
                let viewable = node.access.can_view() && node.parent != Some(BlockParent::Detached);
                let within = asked
                    .within
                    .is_none_or(|within| held.graph.is_within(found.block, within));
                if !viewable || !within {
                    continue;
                }
                (node.content_type, node.head)
            };
            if hits.len() >= SHOWN {
                break;
            }
            let text = cached_text(
                peer,
                shared,
                replicas,
                (found.block, &found.key),
                content_type,
                head,
            )
            .await;
            let snippet = query.snippet(text.as_deref().unwrap_or_default());
            hits.push(SearchHit {
                block: found.block,
                key: found.key,
                snippet: snippet.text,
                matches: snippet.matches,
            });
        }
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let graph = held.graph.revision;
        if let Some(answer) = held.search.answers.get_mut(&asked) {
            answer.shown = Some(Searched {
                hits,
                complete: false,
            });
            answer.missing = missing.then_some(graph);
            held.search.revision += 1;
        }
    }
    be_host_base::host::wake();
    true
}

async fn cached_text(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    (block, key): (Uuid, &str),
    content_type: Uuid,
    head: Option<CommitId>,
) -> Option<String> {
    if let Some(text) = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .search
        .texts
        .get(&(block, key.to_owned(), head))
    {
        return text.clone();
    }
    let text = text_of(peer, shared, replicas, block, content_type, key).await;
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    if held.search.texts.len() >= TEXTS {
        held.search.texts.clear();
    }
    held.search
        .texts
        .insert((block, key.to_owned(), head), text.clone());
    text
}
