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
use be_block_graph::{Access, BlockParent, EDIT_KINDS, SEARCH_INDEX_TYPE, SETTINGS_TYPE};
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

/// How often a commit that cannot be read holds a catch-up's watermark back before it is let
/// go (a history collected under the head it was listed at never reads).
const MAX_READ_FAILURES: u8 = 3;

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
    /// The graph's revision when it was shown: it is shown again once the graph moves.
    missing: Option<u64>,
    /// The graph's revision when it was found without reading every index at the head it is
    /// kept for (an index that did not load, or a newer head that could not be told apart
    /// offline): it runs again once the graph moves, as a reconnect does, not every pass.
    retry: Option<u64>,
}

/// What `wants_index` was worked out at: the graph revision, the sizes of the sets that decide
/// it, and whether the device is connected.
#[derive(Clone, Copy, PartialEq)]
struct WantKey {
    graph: u64,
    creating: usize,
    named: usize,
    failed: usize,
    given_up: usize,
    caught_up: usize,
    catching: usize,
    confirming: usize,
    settle: usize,
    connected: bool,
}

/// A define that was sent: where it is on its way to the files pane's size.
#[derive(Clone, Copy)]
struct Confirm {
    /// The root was read again once the define was sent.
    read: bool,
    /// `Shared::drains` when it was defined: a drain finished after it has sent what it could.
    drains: u64,
}

/// What the indexer needs of a block `changed-since` named, which the mirror may not hold.
#[derive(Clone, Debug)]
struct Caught {
    root: Uuid,
    content_type: Uuid,
    name: String,
    head: Option<CommitId>,
}

impl Settle {
    /// The sequence the index has seen every change up to.
    #[cfg(test)]
    pub(crate) fn sequence(&self) -> u64 {
        self.sequence
    }

    /// The blocks that left its tree.
    #[cfg(test)]
    pub(crate) fn left(&self) -> &[Uuid] {
        &self.left
    }
}

/// What an index is to be given once the blocks a catch-up queued are indexed: the sequence it
/// has now seen every change up to, and the blocks that left its tree.
#[derive(Default)]
pub(crate) struct Settle {
    sequence: u64,
    left: Vec<Uuid>,
    /// The blocks the catch-up queued: the watermark moves only once each is indexed, or the
    /// next catch-up, from the old one, lists them again.
    blocks: BTreeSet<Uuid>,
}

/// What the worker keeps for search, in `Shared::search`.
#[derive(Default)]
pub struct Searching {
    /// The indexes the wanted searches read, which this connection watches: a head another
    /// device moves reaches a device online only through a watch (`Event::HeadChanged`), and
    /// moves the mirror's head the answers are kept for.
    watching: HashSet<Uuid>,
    /// Blocks whose text their root's index has yet to take.
    dirty: BTreeSet<Uuid>,
    /// Indexes a batch is being written to.
    writing: HashSet<Uuid>,
    /// Roots whose index this device made, until their metadata names it.
    creating: HashMap<Uuid, Uuid>,
    /// Indexes made and not yet named in their root's metadata (`name_indexes`).
    made: Vec<(Uuid, Uuid)>,
    /// Blocks that `changed-since` named, for the indexer to read where the mirror does not.
    caught: HashMap<Uuid, Caught>,
    /// Roots whose catch-up runs, and those that ran since this device last connected.
    catching: HashSet<Uuid>,
    caught_up: HashSet<Uuid>,
    /// By index: what it is given when the blocks a catch-up queued for it are done.
    settle: HashMap<Uuid, Vec<Settle>>,
    /// Blocks a batch that was written put, for a settle to check against.
    done: HashSet<Uuid>,
    /// How often a block's commit could not be read: held back a few times, then counted as
    /// put, since a catch-up that lists it again would otherwise never end.
    failures: HashMap<Uuid, u8>,
    /// How many parts the types read to answer `search-text` for indexing, since the start: a
    /// part whose text an index holds is not read again.
    pub(crate) parts_read: u64,
    /// Blocks to read whole next time, passing the type no part as indexed (`reread`).
    whole: HashSet<Uuid>,
    /// Roots just defined, until their define is sent and the mirror has read the root again,
    /// so the size it shows is the server's with the define in it.
    confirming: HashMap<Uuid, Confirm>,
    /// Roots whose index could not be made for want of a connection or a graph: not tried
    /// again until the graph moves past the revision they failed at.
    failed: HashMap<Uuid, u64>,
    /// Roots whose index naming was refused or failed eight times, with the access this
    /// account had then: tried again when the access changes.
    given_up: HashMap<Uuid, Access>,
    /// `wants_index` for the graph and queues it was last worked out at.
    wants: std::cell::Cell<Option<(WantKey, bool)>>,
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
    /// Set by a test that writes as a device without an indexer.
    pub off: bool,
}

impl Searching {
    /// The new connection watches nothing yet.
    pub(crate) fn reconnected(&mut self) {
        self.watching.clear();
    }

    /// Queues a block whose head this device's session moved.
    pub(crate) fn sealed(&mut self, block: Uuid) {
        self.dirty.insert(block);
    }

    /// Queues `blocks` to be read whole, every part of them, since the index lacks text it was
    /// told it held (`write`).
    pub(crate) fn reread(&mut self, blocks: impl IntoIterator<Item = Uuid>) {
        for block in blocks {
            self.dirty.insert(block);
            self.whole.insert(block);
        }
    }

    /// Whether blocks wait to be indexed, an index is being made or named, a batch is being
    /// written or a wanted search is not answered yet. The index counts in the size of the
    /// tree it is under, so a painting taken before it is named would change.
    /// What keeps search busy (`busy`), named for the block store's busy reason.
    pub(crate) fn busy_with(&self, unindexed: bool) -> Option<&'static str> {
        let indexing = !self.off;
        [
            (
                indexing && !self.dirty.is_empty(),
                "edits wait to be indexed",
            ),
            (
                indexing && !self.creating.is_empty(),
                "an index is being made",
            ),
            (
                indexing && !self.confirming.is_empty(),
                "an index waits for its root",
            ),
            (indexing && !self.catching.is_empty(), "an index catches up"),
            (
                indexing && !self.settle.is_empty(),
                "an index waits to settle",
            ),
            (indexing && unindexed, "a workspace waits for its index"),
            (!self.writing.is_empty(), "a batch is written to an index"),
            (!self.running.is_empty(), "a search runs"),
            (
                self.wanted().any(|asked| {
                    self.answers
                        .get(asked)
                        .is_none_or(|answer| answer.shown.is_none())
                }),
                "a search waits to be shown",
            ),
        ]
        .into_iter()
        .find_map(|(busy, reason)| busy.then_some(reason))
    }

    #[cfg(test)]
    pub(crate) fn busy(&self, unindexed: bool) -> bool {
        self.busy_with(unindexed).is_some()
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

    /// An index that could not be made now (a connection, a graph in flux): the root is tried
    /// again when the graph moves past `revision`.
    pub(crate) fn make_failed(&mut self, root: Uuid, revision: u64) {
        self.drop_made(root);
        self.failed.insert(root, revision);
    }

    /// An index that is refused, or could not be named in eight tries: the root is tried again
    /// when this account's `access` to it changes.
    pub(crate) fn give_up(&mut self, root: Uuid, access: Access) {
        self.drop_made(root);
        self.given_up.insert(root, access);
    }

    /// A root with no index to make or name any more: the blocks that waited for it are looked
    /// at again, and find no index to wait for.
    pub(crate) fn drop_made(&mut self, root: Uuid) {
        self.creating.remove(&root);
        self.tries.remove(&root);
        self.made.retain(|(made, _)| *made != root);
        self.confirming.remove(&root);
        let waiting = std::mem::take(&mut self.waiting);
        self.dirty.extend(waiting);
    }

    /// Forgets the roots given up on whose access has changed since (`access_of` is the
    /// graph's), so a root demoted and promoted again is tried again.
    pub(crate) fn prune_given_up(&mut self, access_of: impl Fn(Uuid) -> Option<Access>) {
        self.given_up
            .retain(|root, access| access_of(*root) == Some(*access));
    }

    /// Whether the root is left alone for now, at this graph revision and access.
    pub(crate) fn excluded(&self, root: Uuid, revision: u64, access: Access) -> bool {
        self.failed.get(&root) == Some(&revision) || self.given_up.get(&root) == Some(&access)
    }

    /// The defines to read the root again for: those sent (the outbox is empty). A define
    /// still in the outbox after a drain ended since it was made is stuck, and is not waited
    /// for: the size shown is then the old one.
    pub(crate) fn confirm_due(&mut self, waiting: usize, draining: bool, drains: u64) -> Vec<Uuid> {
        let sent = waiting == 0 && !draining;
        let mut due = Vec::new();
        self.confirming
            .retain(|root, confirm| match (confirm.read, sent) {
                (true, _) => true,
                (false, true) => {
                    confirm.read = true;
                    due.push(*root);
                    true
                }
                (false, false) => draining || drains <= confirm.drains,
            });
        due
    }

    /// The root was read again: its size is the server's.
    pub(crate) fn confirmed(&mut self, root: Uuid) {
        self.confirming.remove(&root);
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

    /// Records a define just sent, when `drains` outbox drains had finished.
    #[cfg(test)]
    pub(crate) fn defined(&mut self, root: Uuid, drains: u64) {
        self.confirming.insert(
            root,
            Confirm {
                read: false,
                drains,
            },
        );
    }

    /// Takes the queued blocks (a test's way to see what `index` would look at).
    #[cfg(test)]
    pub(crate) fn take_queued(&mut self) -> Vec<Uuid> {
        std::mem::take(&mut self.dirty).into_iter().collect()
    }

    /// The settles that may be written, one per index: the pages of a catch-up that are done, in
    /// order, merged. A page with a block that was not put (a commit that could not be fetched, a
    /// type that could not be asked) ends them, and it and the pages after it are dropped: the
    /// next catch-up lists their blocks again from the watermark the last done page left.
    pub(crate) fn settled(&mut self) -> HashMap<Uuid, Settle> {
        let mut ready = HashMap::new();
        for (index, pages) in std::mem::take(&mut self.settle) {
            let mut merged: Option<Settle> = None;
            let mut stopped = false;
            for page in pages {
                let put = !stopped && page.blocks.iter().all(|block| self.done.contains(block));
                for block in &page.blocks {
                    self.done.remove(block);
                }
                stopped |= !put;
                if put {
                    let merged = merged.get_or_insert_with(Settle::default);
                    merged.sequence = page.sequence;
                    merged.left.extend(page.left);
                }
            }
            if let Some(merged) = merged {
                ready.insert(index, merged);
            }
        }
        ready
    }

    /// Records a page of a catch-up (a test's way into `settle`).
    #[cfg(test)]
    pub(crate) fn catch_page(
        &mut self,
        index: Uuid,
        sequence: u64,
        left: Vec<Uuid>,
        blocks: &[Uuid],
    ) {
        self.settle.entry(index).or_default().push(Settle {
            sequence,
            left,
            blocks: blocks.iter().copied().collect(),
        });
    }

    /// Records that a batch put a block.
    #[cfg(test)]
    pub(crate) fn put(&mut self, block: Uuid) {
        self.mark_put(vec![block]);
    }

    /// Records blocks a written batch put, for a page of a catch-up to count: only those a
    /// pending page names, so what live edits put is not kept.
    fn mark_put(&mut self, blocks: Vec<Uuid>) {
        for block in blocks {
            if self
                .settle
                .values()
                .flatten()
                .any(|page| page.blocks.contains(&block))
            {
                self.done.insert(block);
            }
        }
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
    confirm_names(peer, shared, tasks);
    start_catch_ups(peer, shared, tasks);
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
            let settles = match held.search.writing.is_empty() {
                true => held.search.settled(),
                false => HashMap::new(),
            };
            held.search.writing.extend(settles.keys().copied());
            drop(held);
            make_indexes(peer, shared, tasks, roots);
            settle_indexes(peer, shared, tasks, settles);
            return false;
        }
        let dirty = std::mem::take(&mut held.search.dirty);
        let mut batches: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        let mut kept = BTreeSet::new();
        for block in dirty {
            let placed = match (held.graph.get(block), held.search.caught.get(&block)) {
                (None, Some(caught)) => Place::Root(caught.root),
                _ => place(&held.graph, block),
            };
            let root = match placed {
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
        let mut put_blocks = Vec::new();
        // What the index holds now, read once a batch: each block's entries say which documents
        // it answered before and which parts' text is held. An index that cannot be loaded
        // gives no entries, so each block is read whole and removes nothing.
        let content = match head_of(peer, index).await {
            Some(head) => load(peer, head).await.ok(),
            None => None,
        };
        for block in &blocks {
            let whole = shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .search
                .whole
                .remove(block);
            let held = match &content {
                Some(content) => content
                    .entries_of(&PeerParts(peer), block.as_bytes())
                    .await
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            // None: held back (a commit or part that could not be read), so not put.
            match puts_of(peer, shared, replicas, *block, &held, whole).await {
                Some((puts, removed)) => {
                    batch.removed.extend(removed);
                    // Failures count the reads in a row that failed.
                    shared
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .search
                        .failures
                        .remove(block);
                    put_blocks.push(*block);
                    batch.put.extend(puts);
                }
                None => {
                    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                    if whole {
                        held.search.whole.insert(*block);
                    }
                    let failures = held.search.failures.entry(*block).or_default();
                    *failures += 1;
                    if *failures >= MAX_READ_FAILURES {
                        held.search.failures.remove(block);
                        put_blocks.push(*block);
                    }
                }
            }
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
                    Ok(Some(needs_text)) => {
                        for block in &put_blocks {
                            held.search.caught.remove(block);
                        }
                        held.search.mark_put(put_blocks);
                        held.search.answers.clear();
                        held.search.reread(
                            needs_text
                                .iter()
                                .filter_map(|doc| Uuid::from_slice(&doc.block).ok()),
                        );
                    }
                    // Not on this device yet: made by a command still queued.
                    Ok(None) => held.search.park(index, blocks),
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
    let state = &mut *held;
    let graph = &state.graph;
    state
        .search
        .prune_given_up(|root| graph.get(root).map(|node| node.access));
    let roots = unindexed(held);
    roots
        .into_iter()
        .map(|root| {
            let index = Uuid::now_v7();
            held.search.creating.insert(root, index);
            (root, index)
        })
        .collect()
}

/// The workspaces in the mirror this device may edit that name no index and have none being
/// made.
pub(crate) fn unindexed(held: &Shared) -> Vec<Uuid> {
    held.graph
        .nodes()
        .filter(|node| {
            node.parent == Some(BlockParent::Root)
                && node.content_type == be_block::WORKSPACE_TYPE
                && node.access.can_edit()
                && !node.locked
                && !node.metadata.contains(&SEARCH_INDEX)
        })
        .filter(|node| {
            !held.search.creating.contains_key(&node.id)
                && !held.search.named.contains_key(&node.id)
                && !held
                    .search
                    .excluded(node.id, held.graph.revision, node.access)
        })
        .map(|node| node.id)
        .collect()
}

/// Whether some workspace wants an index it has none of, as the worker's `busy` asks: worked
/// out again only when the graph or the sets that decide it changed.
pub(crate) fn wants_index(held: &Shared) -> bool {
    let key = WantKey {
        graph: held.graph.revision,
        creating: held.search.creating.len(),
        named: held.search.named.len(),
        failed: held.search.failed.len(),
        given_up: held.search.given_up.len(),
        caught_up: held.search.caught_up.len(),
        catching: held.search.catching.len(),
        confirming: held.search.confirming.len(),
        settle: held.search.settle.len(),
        connected: held.connected,
    };
    if let Some((cached, wants)) = held.search.wants.get()
        && cached == key
    {
        return wants;
    }
    let wants = !unindexed(held).is_empty() || !catch_ups_due(held, held.connected).is_empty();
    held.search.wants.set(Some((key, wants)));
    wants
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
                        let revision = held.graph.revision;
                        held.search.make_failed(root, revision);
                        if !not_there_yet(&error) {
                            held.error = Some(format!("Making a search index failed: {error}"));
                        }
                    }
                }
            }
            be_host_base::host::wake();
        });
    }
}

/// What a block's documents hold at its head: its name, and each document its type answers
/// `search-text` with. A type that answers nothing is found by its name alone.
/// A block's documents to put at its head, and those the index holds for it that its type no
/// longer answers, to remove. `held` is what the index holds for the block: the parts whose text
/// it has are handed to the type as indexed, so it does not read them again.
async fn puts_of(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    block: Uuid,
    held: &[be_lib_search::Entry],
    whole: bool,
) -> Option<(Vec<Put>, Vec<(DocId, be_lib_search::Commit)>)> {
    let Some((content_type, name, head)) = ({
        let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        match held.graph.get(block) {
            Some(node) => Some((
                node.content_type,
                node.metadata.name().unwrap_or_default(),
                node.head,
            )),
            None => held
                .search
                .caught
                .get(&block)
                .map(|caught| (caught.content_type, caught.name.clone(), caught.head)),
        }
    }) else {
        // Gone from the mirror and from what a catch-up named: nothing to index.
        return Some((Vec::new(), Vec::new()));
    };
    let caught_head = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .search
        .caught
        .get(&block)
        .and_then(|caught| caught.head);
    // A block a catch-up named is read at the head it was listed at, unless this device holds
    // a newer one (an edit not sent yet); a local head that lags the server's is not used, and
    // a local head that is not a descendant (a concurrent edit not sent) is indexed once it is
    // sent. The walk of the local head's history is what it costs.
    let head = match (peer.local_head(block), caught_head) {
        (Some(local), Some(listed)) if local != listed => {
            match peer.descends(local, listed).await {
                Ok(true) => Some(local),
                _ => Some(listed),
            }
        }
        (Some(local), _) => Some(local),
        (None, listed) => listed.or(head),
    };
    let Some(head) = head else {
        // A block no one has written to has nothing to index.
        return Some((Vec::new(), Vec::new()));
    };
    // A commit that cannot be fetched now (a connection, a key) is held back: the block is not
    // put, and a catch-up that listed it does not move its watermark past it.
    let Ok(commit) = peer.load_commit(head).await else {
        return None;
    };
    let at = be_lib_search::Commit {
        time: commit.time,
        id: *head.hash().as_bytes(),
    };
    // Only a module already running is asked: starting one would hold up the worker's loop.
    // A block a catch-up named may be of a type this device has not run yet: its module is
    // started once, which is worth it to index what other devices wrote.
    let from_catch_up = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .search
        .caught
        .contains_key(&block);
    let known = match from_catch_up {
        true => replicas.kind(content_type).await.is_some(),
        false => crate::kind_of(content_type).is_some(),
    };
    let indexed: Vec<[u8; 32]> = held
        .iter()
        .filter(|entry| !whole && !entry.is_removed())
        .filter_map(|entry| entry.source)
        .collect();
    // Whether the type answered every document: only then is one it no longer answers removed.
    let (mut documents, answered) = match known {
        true => {
            match documents_of(
                peer,
                shared,
                replicas,
                content_type,
                &commit,
                None,
                &indexed,
            )
            .await?
            {
                Some(documents) => (documents, true),
                None => (Vec::new(), false),
            }
        }
        false => (Vec::new(), false),
    };
    if !documents.iter().any(|document| document.key.is_empty()) {
        documents.push(be_sdk_type_api::SearchDocument {
            key: String::new(),
            source: None,
            name: None,
            body: Some(String::new()),
        });
    }
    let removed = match answered {
        true => {
            let answers: HashSet<&str> = documents
                .iter()
                .map(|document| document.key.as_str())
                .collect();
            held.iter()
                .filter(|entry| !entry.is_removed() && !answers.contains(entry.doc.key.as_str()))
                .map(|entry| (entry.doc.clone(), at))
                .collect()
        }
        false => Vec::new(),
    };
    let puts = documents
        .into_iter()
        .map(|document| {
            let text = match (document.key.is_empty(), document.body) {
                (true, body) => Some(Text {
                    name: name.clone(),
                    body: capped(body.unwrap_or_default()),
                }),
                (false, Some(body)) => Some(Text {
                    name: document.name.unwrap_or_default(),
                    body: capped(body),
                }),
                // Its source is a part whose text the index holds.
                (false, None) => None,
            };
            Put {
                doc: DocId {
                    block: block.as_bytes().to_vec(),
                    key: document.key,
                },
                commit: at,
                source: document
                    .source
                    .map(|source| *be_block_store::PartRef::from(source).hash().as_bytes()),
                text,
            }
        })
        .collect();
    Some((puts, removed))
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
/// some are named; no body for one whose source is in `indexed`, part hashes): `Some(None)`
/// when the type breaks the rules or cannot be asked, so the block is found by its name alone,
/// and None when a part could not be read.
async fn documents_of(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    content_type: Uuid,
    commit: &be_block_commit::Commit,
    keys: Option<Vec<String>>,
    indexed: &[[u8; 32]],
) -> Option<Option<Vec<be_sdk_type_api::SearchDocument>>> {
    match replicas
        .search_text(
            (peer, shared),
            content_type,
            &commit.manifest,
            keys,
            indexed,
        )
        .await
    {
        Ok(searched) => {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .search
                .parts_read += searched.read.len() as u64;
            Some(Some(searched.documents))
        }
        // A part that could not be read (offline, a key): held back, as a commit is.
        Err(be_block_type_host::NotSearched::Read(_)) => None,
        Err(_) => Some(None),
    }
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
    documents_of(peer, shared, replicas, content_type, &commit, keys, &[])
        .await??
        .into_iter()
        .find(|document| document.key == key)?
        .body
}

/// Makes `root`'s index, empty: in a task, beside the worker's loop, offline-first like any
/// create; `name_indexes` then names it in the root's metadata.
pub(crate) async fn make_index<S: ObjectStore>(
    peer: &Peer<S>,
    root: Uuid,
    index: Uuid,
) -> Result<(), be_block_client::ClientError> {
    peer.create_block(
        index,
        SEARCH_INDEX_TYPE,
        BlockParent::Block(root),
        &BlockMetadata::default(),
        false,
    )
    .await?;
    let empty = SearchIndexContent::default();
    let write = Write::encoded(SEARCH_INDEX_TYPE, empty.parts(), None)
        .described(Vec::new(), be_block::derived_metadata(&empty));
    peer.seal(index, write).await?;
    Ok(())
}

/// A refusal that passes by itself: the server does not hold the root yet (its own create
/// still on its way), so the index is made again when the mirror next changes, and no error
/// is shown.
pub(crate) fn not_there_yet(error: &be_block_client::ClientError) -> bool {
    matches!(
        error,
        be_block_client::ClientError::Refused(be_block_protocol::ErrorCode::BlockNotFound, _)
    )
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
            held.search.drop_made(root);
        }
        let (ready, waiting): (Vec<_>, Vec<_>) = named.into_iter().partition(|(root, _)| {
            replicas
                .status(*root)
                .is_none_or(|status| status.clean && !status.owes_seal)
        });
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
                if peer.is_attached() {
                    let drains = held.drains;
                    held.search.confirming.insert(
                        root,
                        Confirm {
                            read: false,
                            drains,
                        },
                    );
                }
                held.graph.update(root, |node| {
                    node.metadata.insert(&SEARCH_INDEX, &BlockRef(index));
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
                        let access = held
                            .graph
                            .get(root)
                            .map_or(Access::default(), |node| node.access);
                        held.search.give_up(root, access);
                        held.error = Some(format!("Naming a search index failed: {error}"));
                    }
                    false => held.search.made.push((root, index)),
                }
            }
        }
    }
}

/// Starts the catch-up of each root this device may edit whose index is named and has not
/// been caught up since this device connected (guides/blocks/host.md, "Catching up"): commits
/// from devices that never index, and blocks that left the tree, are found by what
/// `changed-since` lists after the index's watermark.
fn start_catch_ups(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    if held.search.off {
        return;
    }
    if !peer.is_attached() || !held.connected {
        held.search.caught_up.clear();
        return;
    }
    let due = catch_ups_due(&held, true);
    for (root, index) in due {
        held.search.catching.insert(root);
        let (peer, shared, tasks_held) = (Arc::clone(peer), Arc::clone(shared), tasks.clone());
        tasks.spawn(async move {
            let caught = catch_up(&peer, &shared, root, index).await;
            {
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                held.search.catching.remove(&root);
                match caught {
                    Ok(true) => {
                        held.search.caught_up.insert(root);
                    }
                    // No head yet: tried again when the graph next moves.
                    Ok(false) => {
                        let revision = held.graph.revision;
                        held.search.failed.insert(root, revision);
                    }
                    Err(error) if error.is_unreachable() => {}
                    Err(error) => {
                        held.search.caught_up.insert(root);
                        held.error = Some(format!("Catching up a search index failed: {error}"));
                    }
                }
            }
            tasks_held.sync(&peer, &shared);
            be_host_base::host::wake();
        });
    }
}

/// The roots a catch-up is due for: this device may edit them, their index is named and has not
/// been caught up since it connected. None while it is not connected.
pub(crate) fn catch_ups_due(held: &Shared, attached: bool) -> Vec<(Uuid, Uuid)> {
    if held.search.off || !attached {
        return Vec::new();
    }
    held.graph
        .nodes()
        .filter(|node| {
            node.parent == Some(BlockParent::Root)
                && node.content_type == be_block::WORKSPACE_TYPE
                && node.access.can_edit()
                && !node.locked
                && !held
                    .search
                    .excluded(node.id, held.graph.revision, node.access)
        })
        .filter_map(|node| {
            let index = index_of(&held.graph, &held.search.named, node.id)?;
            let idle = !held.search.catching.contains(&node.id)
                && !held.search.caught_up.contains(&node.id)
                && !held.search.creating.contains_key(&node.id)
                && !held.search.confirming.contains_key(&node.id);
            idle.then_some((node.id, index))
        })
        .collect()
}

/// Catches `root`'s index up, a page of `changed-since` at a time from its watermark: each
/// page queues the blocks it lists and leaves a settle with the watermark it reaches (all
/// before the page's last sequence, which the next page may share), so a catch-up that is
/// interrupted resumes from the last page that was indexed. False while the index has no head
/// here.
async fn catch_up(
    peer: &Peer<Store>,
    shared: &Arc<Mutex<Shared>>,
    root: Uuid,
    index: Uuid,
) -> Result<bool, be_block_client::ClientError> {
    const PAGE: u32 = 200;
    let Some(head) = head_of(peer, index).await else {
        return Ok(false);
    };
    let Ok(content) = load(peer, head).await else {
        return Ok(false);
    };
    let since = content.index().watermark();
    let mut after = be_block_protocol::ChangeCursor {
        changed: since,
        block: Uuid::max(),
    };
    loop {
        let page = peer.changed_page(root, after, PAGE).await?;
        peer.learn(&page.summaries).await?;
        let blocks: Vec<(Uuid, Caught)> = page
            .summaries
            .iter()
            .filter(|summary| {
                summary.id != root
                    && summary.access.can_view()
                    && summary.content_type != SEARCH_INDEX_TYPE
                    && summary.content_type != SETTINGS_TYPE
                    && !EDIT_KINDS.contains(&summary.content_type)
            })
            .map(|summary| {
                let name = peer
                    .metadata(summary)
                    .ok()
                    .and_then(|metadata| metadata.name())
                    .unwrap_or_default();
                (
                    summary.id,
                    Caught {
                        root,
                        content_type: summary.content_type,
                        name,
                        head: summary.head,
                    },
                )
            })
            .collect();
        // Everything before the cursor's sequence is in this page or an earlier one; the
        // sequence itself may go on in the next.
        let reached = match &page.next {
            Some(next) => next.changed.saturating_sub(1).max(since),
            None => page.sequence,
        };
        {
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            // What a live edit put before is not what this page lists.
            for (block, _) in &blocks {
                held.search.done.remove(block);
            }
            held.search.settle.entry(index).or_default().push(Settle {
                sequence: reached,
                left: page.removed,
                blocks: blocks.iter().map(|(block, _)| *block).collect(),
            });
            for (block, caught) in blocks {
                held.search.caught.insert(block, caught);
                held.search.dirty.insert(block);
            }
        }
        be_host_base::host::wake();
        match page.next {
            Some(next) => after = next,
            None => return Ok(true),
        }
    }
}

/// Gives each index the watermark of its catch-up and the blocks that left its tree, in one
/// batch, once everything the catch-up queued is indexed.
fn settle_indexes(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: &Tasks,
    settles: HashMap<Uuid, Settle>,
) {
    for (index, settle) in settles {
        let (peer, shared, tasks_held) = (Arc::clone(peer), Arc::clone(shared), tasks.clone());
        tasks.spawn(async move {
            let at = be_lib_search::Commit {
                time: be_block_commit::now_milliseconds(),
                id: [0; 32],
            };
            // Only what the index holds is masked: the call names every block that left the
            // tree, and an index that never had one has nothing of it to hide.
            let held = held_blocks(&peer, index, &settle.left).await;
            let batch = Batch {
                removed_blocks: held
                    .iter()
                    .map(|block| (block.as_bytes().to_vec(), at))
                    .collect(),
                watermark: Some(settle.sequence),
                ..Batch::default()
            };
            let written = write(&peer, index, batch).await;
            tasks_held.sync(&peer, &shared);
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            held.search.writing.remove(&index);
            match written {
                Ok(Some(needs_text)) => {
                    held.search.answers.clear();
                    held.search.reread(
                        needs_text
                            .iter()
                            .filter_map(|doc| Uuid::from_slice(&doc.block).ok()),
                    );
                }
                Ok(None) => {}
                Err(error) => held.error = Some(format!("Indexing for search failed: {error}")),
            }
            drop(held);
            be_host_base::host::wake();
        });
    }
}

/// Which of `blocks` the index holds a document of.
async fn held_blocks(peer: &Peer<Store>, index: Uuid, blocks: &[Uuid]) -> Vec<Uuid> {
    let Some(head) = head_of(peer, index).await else {
        return Vec::new();
    };
    let Ok(content) = load(peer, head).await else {
        return Vec::new();
    };
    let mut held = Vec::new();
    for block in blocks {
        let entries = content.entries_of(&PeerParts(peer), block.as_bytes()).await;
        if entries.is_ok_and(|entries| entries.iter().any(|entry| !entry.is_removed())) {
            held.push(*block);
        }
    }
    held
}

/// Once a define is sent the root is read again, whatever order the server's own news of it
/// came in, and the host is busy until it has been; a define that stays in the outbox after a
/// drain, or a lost connection, is not waited for.
fn confirm_names(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    if held.search.confirming.is_empty() {
        return;
    }
    if !peer.is_attached() {
        held.search.confirming.clear();
        return;
    }
    let (waiting, draining, drains) = (held.waiting, held.draining, held.drains);
    for root in held.search.confirm_due(waiting, draining, drains) {
        let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
        tasks.spawn(async move {
            crate::worker::refresh(&peer, &shared, root).await;
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .search
                .confirmed(root);
            be_host_base::host::wake();
        });
    }
}

/// Reads a range of a saved part through a peer.
struct PeerParts<'a, S: ObjectStore>(&'a Peer<S>);

impl<S: ObjectStore> AsyncRead for PeerParts<'_, S> {
    fn read(
        &self,
        part: PartRef,
        range: Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + '_>> {
        Box::pin(async move {
            self.0
                .read_part(part, range.start, range.end - range.start)
                .await
                .map_err(|error| error.to_string())
        })
    }
}

/// An index's newest head this device knows: its own unsent one, else the server's, else the
/// one it last saw.
/// The head a batch applies to: the newest this device knows, so indexing never waits on the
/// server; a batch on a head older than the server's meets the newer one as a diverged block and
/// merges by parts. The server is asked only when the device knows no head.
async fn head_of<S: ObjectStore>(peer: &Peer<S>, index: Uuid) -> Option<CommitId> {
    // What this device sealed and sent is its local head before the server's word of it moves
    // the remote head: a batch written then reads the newer, rather than the one it replaced.
    let local = peer.local_head(index);
    let known = match (peer.pending_commits(index).is_empty(), local, peer.known_remote_head(index)) {
        (false, _, _) => local,
        (true, Some(local), Some(remote)) if local != remote => {
            match peer.descends(local, remote).await {
                Ok(true) => Some(local),
                _ => Some(remote),
            }
        }
        (true, local, remote) => remote.or(local),
    };
    if known.is_some() {
        return known;
    }
    peer.newest_head(index).await.ok().flatten()
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

/// What `index` holds for `block` at its newest head.
#[cfg(test)]
pub(crate) async fn entries<S: ObjectStore>(
    peer: &Peer<S>,
    index: Uuid,
    block: Uuid,
) -> Vec<be_lib_search::Entry> {
    let Some(head) = head_of(peer, index).await else {
        return Vec::new();
    };
    let Ok(content) = load(peer, head).await else {
        return Vec::new();
    };
    content
        .entries_of(&PeerParts(peer), block.as_bytes())
        .await
        .unwrap_or_default()
}

/// Applies a batch to the index's newest head and seals the commit into the outbox, answering
/// the documents the index lacks text for (a put without text whose entry is gone or holds
/// another part's: `held` was read from an older head), for the indexer to read whole again.
/// None when the index has no head on this device yet.
pub async fn write<S: ObjectStore>(
    peer: &Peer<S>,
    index: Uuid,
    mut batch: Batch,
) -> Result<Option<Vec<DocId>>, String> {
    let Some(head) = head_of(peer, index).await else {
        return Ok(None);
    };
    let mut content = load(peer, head).await?;
    outdate_ancestors(peer, &content, &mut batch).await?;
    let needs_text = content
        .apply(&PeerParts(peer), Uuid::new_v4().into_bytes(), batch)
        .await
        .map_err(|error| error.to_string())?;
    let write = Write::encoded(SEARCH_INDEX_TYPE, content.parts(), Some(head))
        .described(Vec::new(), be_block::derived_metadata(&content));
    peer.seal(index, write)
        .await
        .map_err(|error| error.to_string())?;
    Ok(Some(needs_text))
}

/// A put or removal wins over the entry the index holds for its document only when its commit
/// is the later: by time, then by id. Commits made in the same millisecond tie on time and the
/// id is arbitrary, so a change that descends from the held commit takes the time after it: a
/// descendant is always the newer.
async fn outdate_ancestors<S: ObjectStore>(
    peer: &Peer<S>,
    content: &SearchIndexContent,
    batch: &mut Batch,
) -> Result<(), String> {
    // Each block's entries by key, read once however many of its documents the batch changes.
    let mut entries: HashMap<Vec<u8>, HashMap<String, be_lib_search::Entry>> = HashMap::new();
    let changes = batch
        .put
        .iter_mut()
        .map(|put| (&put.doc, &mut put.commit))
        .chain(
            batch
                .removed
                .iter_mut()
                .map(|(doc, commit)| (&*doc, commit)),
        );
    for (doc, commit) in changes {
        if !entries.contains_key(&doc.block) {
            let read = content
                .entries_of(&PeerParts(peer), &doc.block)
                .await
                .map_err(|error| error.to_string())?;
            entries.insert(
                doc.block.clone(),
                read.into_iter()
                    .map(|entry| (entry.doc.key.clone(), entry))
                    .collect(),
            );
        }
        // A block that left and came back is put at a commit the index masked: it is stamped
        // after the removal, which is only an order.
        if let Some(removal) = content.index().removed_at(&doc.block)
            && removal >= *commit
        {
            commit.time = removal.time.saturating_add(1);
        }
        let Some(old) = entries[&doc.block].get(&doc.key) else {
            continue;
        };
        if old.commit < *commit || old.commit.id == commit.id {
            continue;
        }
        let (old_id, new_id) = (
            CommitId::from_hash(be_block_store::Hash::from_bytes(old.commit.id)),
            CommitId::from_hash(be_block_store::Hash::from_bytes(commit.id)),
        );
        if peer
            .descends(new_id, old_id)
            .await
            .map_err(|error| error.to_string())?
        {
            commit.time = old.commit.time.saturating_add(1);
        }
    }
    Ok(())
}

/// The best hits of `query` in one index, at its newest head.
/// The head of `index` a search reads, and whether it is surely the newest this device could
/// read: `head_of`'s, unless `listed`, the head the graph mirror last heard of, is newer. A
/// watch moves the mirror's head when another device publishes, before this device fetches
/// anything, while the remote head `head_of` reads moves only when this device publishes or
/// catches up itself. Whichever descends from the other is the newer. Two heads where neither
/// descends from the other have diverged: this device's merge of them is on its way, and until
/// it lands `head_of`'s, the one this device holds, is read. Telling them apart walks the
/// listed head's history back to the other's (`Peer::descends` fetches what it lacks), once per
/// new head: an index's history grows by one commit a batch, and what was fetched is kept.
async fn search_head<S: ObjectStore>(
    peer: &Peer<S>,
    index: Uuid,
    listed: Option<CommitId>,
) -> (Option<CommitId>, bool) {
    let known = head_of(peer, index).await;
    match (listed, known) {
        (Some(listed), Some(known))
            if listed != known && peer.pending_commits(index).is_empty() =>
        {
            match peer.descends(listed, known).await {
                Ok(true) => (Some(listed), true),
                Ok(false) => (Some(known), true),
                // Offline, or the listed head not fetched: read what is held, and again later.
                Err(_) => (Some(known), false),
            }
        }
        (listed, known) => (known.or(listed), true),
    }
}

pub async fn search_index<S: ObjectStore>(
    peer: &Peer<S>,
    index: Uuid,
    listed: Option<CommitId>,
    query: &Query,
    limit: usize,
) -> Result<Vec<be_lib_search::Hit>, String> {
    let (Some(head), _) = search_head(peer, index, listed).await else {
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
    let mut searched = HashSet::new();
    let mut started = false;
    for asked in wanted {
        let indexes = indexes(&held.graph, &held.search.named, asked.within);
        searched.extend(indexes.iter().copied());
        followed.extend(indexes.iter().map(|index| GraphQuery::Block(*index)));
        let read = heads(peer, &held.graph, &indexes);
        if held.search.running.contains(&asked)
            || held.search.answers.get(&asked).is_some_and(|answer| {
                answer.read == read && answer.retry.is_none_or(|at| at == graph)
            })
        {
            continue;
        }
        held.search.running.insert(asked.clone());
        started = true;
        let indexes: Vec<(Uuid, Option<CommitId>)> = indexes
            .iter()
            .map(|index| (*index, held.graph.get(*index).and_then(|node| node.head)))
            .collect();
        let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
        tasks.spawn(async move {
            let (found, exact) = run(&peer, &asked, &indexes).await;
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
                        retry: (!exact).then_some(graph),
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
    let held = &mut *held;
    watch(peer, &held.graph, &mut held.search.watching, &searched);
    let mut seen = HashSet::new();
    followed.retain(|query| seen.insert(*query));
    if held.graph.wanted(Owner::Search) != followed.as_slice() {
        held.graph.want(Owner::Search, followed);
    }
    started
}

/// Watches the indexes in `read` and lets go of the rest, while the connection is attached.
/// An index is watched only once the server holds it: one this device made and has not sent
/// yet would be refused, and a refused watch is not asked again. The server holds it when this
/// device published or caught it up (a remote head it keeps), or when this device holds none of
/// it and the mirror lists a head, which only the server can have told a device that never
/// wrote it (a device that only reads, or one new to the account).
fn watch(peer: &Peer<Store>, graph: &Graph, watching: &mut HashSet<Uuid>, read: &HashSet<Uuid>) {
    let attached = peer.is_attached();
    let read: HashSet<Uuid> = read
        .iter()
        .copied()
        .filter(|index| {
            peer.known_remote_head(*index).is_some()
                || (peer.local_head(*index).is_none()
                    && graph.get(*index).is_some_and(|node| node.head.is_some()))
        })
        .collect();
    let read = &read;
    let gone: Vec<Uuid> = watching.difference(read).copied().collect();
    for index in gone {
        watching.remove(&index);
        if attached {
            let _ = peer.unwatch(index);
        }
    }
    if attached {
        for index in read {
            if watching.insert(*index) {
                peer.watch(*index);
            }
        }
    }
}

/// Every index's best hits, one per document: where two indexes hold a document, the later
/// commit. Each index is searched at the head the graph listed when the search started
/// (`listed`), which is what its answer is kept for (`heads`).
async fn run(
    peer: &Peer<Store>,
    asked: &SearchAsk,
    indexes: &[(Uuid, Option<CommitId>)],
) -> (Vec<Found>, bool) {
    let query = Query::parse(&asked.text);
    let mut best: HashMap<DocId, (f64, be_lib_search::Commit)> = HashMap::new();
    let mut exact = true;
    for (index, listed) in indexes {
        let (head, newest) = search_head(peer, *index, *listed).await;
        exact &= newest;
        let Some(head) = head else {
            continue;
        };
        let hits = match load(peer, head).await {
            Ok(content) => content
                .search(&PeerParts(peer), &query, HITS)
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error),
        };
        let Ok(hits) = hits else {
            exact = false;
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
    (found.into_iter().map(|(_, found)| found).collect(), exact)
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
        for found in found {
            let (content_type, head) = {
                let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(node) = held.graph.get(found.block) else {
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
            // Shown again whenever the mirror moves: a hit may have left the tree, or been moved
            // out of what the search is within, or the mirror may now hold one it lacked.
            answer.missing = Some(graph);
            crate::changes::post(crate::Changes {
                search: true,
                ..crate::Changes::default()
            });
            held.progressed += 1;
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
