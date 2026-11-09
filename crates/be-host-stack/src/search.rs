//! Full-text search (plans/search.md): each root block's search index, which the devices that
//! edit its tree write, and the searches the host and its apps ask for.
//!
//! **Indexing.** A head this device's session moved (`Host::status` while it owns the session)
//! queues its block (`Searching::dirty`). Each round of the worker's loop (`step`) takes the
//! queued blocks whose root it may edit, reads their text with the type's plain file
//! (`to-file`, until types answer `search-text`) and the name from the mirror, and hands each
//! index one batch in a task (`write`), which applies it to the index's local head and seals
//! the commit into the outbox, so it is saved offline-first and published by the next drain;
//! two devices' batches meet as a diverged block and merge by parts. A root with no index gets
//! one, made under it and named by its `SEARCH_INDEX` metadata.
//!
//! **Searching.** `want` names the searches an owner shows. A search runs in a task over the
//! index of every root the account views in the open organization (`run`), reading only the
//! parts it needs; the loop then cuts each shown hit's snippet from its block's text, kept per
//! block and head, and answers. A search runs again when an index it read moves.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    hash::{Hash as _, Hasher as _},
    ops::Range,
    sync::{Arc, PoisonError},
};

use be_block::{
    BlockContent, BlockMetadata, SEARCH_INDEX, be_block_model::BlockRef,
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

/// Texts kept for snippets, by block and head, before the oldest are let go.
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
    wanted: HashMap<Owner, Vec<SearchAsk>>,
    answers: HashMap<SearchAsk, Answer>,
    running: HashSet<SearchAsk>,
    texts: HashMap<(Uuid, Option<CommitId>), Option<String>>,
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

    pub(crate) fn busy(&self) -> bool {
        (!self.off && !self.dirty.is_empty())
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
        if self.wanted.get(&owner) == Some(&asked) {
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
        shown.complete = self.dirty.is_empty() && self.writing.is_empty();
        Some(shown)
    }
}

/// The document a whole block is.
fn doc(block: Uuid) -> DocId {
    DocId {
        block: block.as_bytes().to_vec(),
        key: String::new(),
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

fn index_of(graph: &Graph, root: Uuid) -> Option<Uuid> {
    graph
        .get(root)?
        .metadata
        .get(&SEARCH_INDEX)
        .map(|BlockRef(index)| index)
}

/// One round of the worker's loop: batches for the indexes, searches to run, snippets to cut.
/// Answers whether anything changed.
pub(crate) async fn step(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    tasks: &Tasks,
) -> bool {
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
        if held.search.off || held.search.dirty.is_empty() {
            return false;
        }
        let dirty = std::mem::take(&mut held.search.dirty);
        let mut batches: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        let mut kept = BTreeSet::new();
        let mut roots = Vec::new();
        for block in dirty {
            let root = match place(&held.graph, block) {
                Place::Unknown if held.graph.fetching() => {
                    kept.insert(block);
                    continue;
                }
                Place::Unknown | Place::Skip => continue,
                Place::Root(root) => root,
            };
            if !held.graph.get(root).is_some_and(|root| root.access.can_edit()) {
                continue;
            }
            match index_of(&held.graph, root) {
                Some(index) if held.search.writing.contains(&index) => {
                    kept.insert(block);
                }
                Some(index) => {
                    held.search.creating.remove(&root);
                    batches.entry(index).or_default().push(block);
                }
                None => match held.search.creating.get(&root) {
                    // Made, and still not named by its root: the root's metadata was refused.
                    Some(index) if peer.local_head(*index).is_some() => {}
                    Some(_) => {
                        kept.insert(block);
                    }
                    None => {
                        let index = Uuid::now_v7();
                        held.search.creating.insert(root, index);
                        roots.push((root, index));
                        kept.insert(block);
                    }
                },
            }
        }
        held.search.dirty = kept;
        held.search.writing.extend(batches.keys().copied());
        (batches, roots)
    };
    for (root, index) in roots {
        crate::create(
            index,
            SEARCH_INDEX_TYPE,
            BlockParent::Block(root),
            BlockMetadata::default(),
            Some(SearchIndexContent::default().encode()),
            false,
        );
        crate::define(root, vec![SEARCH_INDEX.definition(Some(&BlockRef(index)))]);
    }
    let moved = !batches.is_empty();
    for (index, blocks) in batches {
        let mut batch = Batch::default();
        for block in &blocks {
            if let Some(put) = put_of(peer, shared, replicas, *block).await {
                batch.put.push(put);
            }
        }
        let (peer, shared, tasks_held) = (Arc::clone(peer), Arc::clone(shared), tasks.clone());
        tasks.spawn(async move {
            let written = write(&peer, index, batch).await;
            {
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                held.search.writing.remove(&index);
                match written {
                    Ok(true) => {}
                    // Not on this device yet: made by a command still queued.
                    Ok(false) => held.search.dirty.extend(blocks),
                    Err(error) => held.error = Some(format!("Indexing for search failed: {error}")),
                }
            }
            tasks_held.sync(&peer, &shared);
            be_host_base::host::wake();
        });
    }
    moved
}

/// What a block's document holds now: its name and the text of its plain file, at its head.
async fn put_of(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    block: Uuid,
) -> Option<Put> {
    let (content_type, name, head) = {
        let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let node = held.graph.get(block)?;
        (
            node.content_type,
            node.metadata.name().unwrap_or_default(),
            node.head,
        )
    };
    let head = peer.local_head(block).or(head)?;
    let commit = peer.load_commit(head).await.ok()?;
    let body = text_of(peer, shared, replicas, block, content_type)
        .await
        .unwrap_or_default();
    Some(Put {
        doc: doc(block),
        commit: be_lib_search::Commit {
            time: commit.time,
            id: *head.hash().as_bytes(),
        },
        source: None,
        text: Some(Text { name, body }),
    })
}

/// A block's searchable text: its plain file, when that is text. Until types answer
/// `search-text` (plans/search_library.md), this is how text and markdown are read.
async fn text_of(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    block: Uuid,
    content_type: Uuid,
) -> Option<String> {
    let open = match replicas.is_open(block) {
        true => replicas.content((peer, shared), block).await,
        false => None,
    };
    let bytes = match open {
        Some(bytes) => bytes,
        None => crate::worker::stored(peer, block).await.ok()??,
    };
    let file = replicas
        .to_file((peer, shared), content_type, bytes)
        .await
        .ok()??;
    file.media_type
        .starts_with("text/")
        .then(|| String::from_utf8_lossy(&file.bytes).into_owned())
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
    batch: Batch,
) -> Result<bool, String> {
    let Some(head) = head_of(peer, index).await else {
        return Ok(false);
    };
    let mut content = load(peer, head).await?;
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
fn indexes(graph: &Graph, within: Option<Uuid>) -> Vec<Uuid> {
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
        .filter_map(|root| index_of(graph, root))
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
        graph.get(*index).and_then(|node| node.head).hash(&mut hasher);
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
        let indexes = indexes(&held.graph, asked.within);
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
                let viewable =
                    node.access.can_view() && node.parent != Some(BlockParent::Detached);
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
            let text = cached_text(peer, shared, replicas, found.block, content_type, head).await;
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
    block: Uuid,
    content_type: Uuid,
    head: Option<CommitId>,
) -> Option<String> {
    if let Some(text) = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .search
        .texts
        .get(&(block, head))
    {
        return text.clone();
    }
    let text = text_of(peer, shared, replicas, block, content_type).await;
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    if held.search.texts.len() >= TEXTS {
        held.search.texts.clear();
    }
    held.search.texts.insert((block, head), text.clone());
    text
}
