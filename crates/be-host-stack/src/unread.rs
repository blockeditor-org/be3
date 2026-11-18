//! Unread counts on the graph's nodes (guides/blocks/host.md, "Unread counts"): for a block
//! whose type counts what the account has not read (`type-info.reads`,
//! `be_block_content::Unread`), the worker loads its head as the type loads it (a chat: only its
//! header) and asks its module, with the block's own place in the account's layers of the
//! type's reads kind, which the page reads from the account's settings and sends here
//! (`publish`). The count lands on the
//! node (`Node::unread`), and so on every row an app draws of the block (`BlockInfo::unread`).
//! No replica is opened for it: one load per head, and one call per head or move of the block's
//! own place in the reads, which the module picks out of them (`Replicas::places`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use be_block_client::Peer;
use be_block_commit::CommitId;
use uuid::Uuid;

use crate::replicas::Replicas;
use crate::worker::{Shared, Store, Tasks};

/// Each reads kind's layers as the page last sent them.
type Layers = BTreeMap<Uuid, Vec<Vec<u8>>>;

static PUBLISHED: Mutex<Option<Layers>> = Mutex::new(None);

/// The reads kinds of the types the running stack's modules know (`KnownKind::reads`): the
/// kinds the page reads the account's layers of and `publish`es.
pub fn kinds() -> Vec<Uuid> {
    let mut kinds: Vec<Uuid> = crate::with_stack(|stack| {
        stack
            .view
            .kinds
            .values()
            .flatten()
            .filter_map(|kind| kind.reads)
            .collect()
    })
    .unwrap_or_default();
    kinds.sort();
    kinds.dedup();
    kinds
}

/// The page's part: the account's layers of the reads kind `kind`, each its content's bytes,
/// lowest first (every device, then this device), sent to the stack's worker when they changed.
pub fn publish(kind: Uuid, layers: Vec<Vec<u8>>) {
    let mut held = PUBLISHED.lock().unwrap_or_else(PoisonError::into_inner);
    let published = held.get_or_insert_with(Layers::new);
    if published.get(&kind) == Some(&layers) {
        return;
    }
    published.insert(kind, layers.clone());
    drop(held);
    crate::send(crate::worker::Command::Reads(kind, layers));
}

/// What `publish` sent last, for a stack that starts.
pub(crate) fn current() -> Layers {
    PUBLISHED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_default()
}

/// Forgets what was published, as the account signs out: the next stack counts nothing until
/// the page publishes its account's reads.
pub fn forget() {
    *PUBLISHED.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

/// A count made: at which head, with which place.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Counted {
    head: CommitId,
    place: Option<Vec<u8>>,
    count: u32,
}

/// The worker's part, in `Shared`: the reads the page sent, each block's place in them, and what
/// was counted with those. A pass looks only at the blocks that moved (`dirty`): a node that
/// moved in the graph (`Graph::take_to_count`), a place that moved, a head loaded.
#[derive(Default)]
pub(crate) struct Counting {
    /// Each reads kind's layers, and the number of the change that set them.
    reads: HashMap<Uuid, (u64, Vec<Vec<u8>>)>,
    changes: u64,
    /// Each counting type's places by block, as its module picked them out of the reads, with
    /// the change of the reads they were picked from. A type counts once it has them.
    places: HashMap<Uuid, (u64, HashMap<Uuid, Vec<u8>>)>,
    /// The reads' change and the known kinds' revision the places were last brought up to.
    placed: Option<(u64, u64)>,
    counted: HashMap<Uuid, Counted>,
    /// Each node's head as its type loads it, kept so a count with a new place loads nothing.
    heads: HashMap<Uuid, (CommitId, be_sdk_type_api::Loaded)>,
    /// Heads being loaded.
    loading: HashSet<Uuid>,
    /// Counts that could not be made (no key, offline, a type that failed): not tried again
    /// until the head or the place moves, or the device connects again (`reconnected`).
    failed: HashMap<Uuid, (CommitId, Option<Vec<u8>>)>,
    /// Blocks to look at again.
    dirty: HashSet<Uuid>,
    /// Whether to look at every node: the known kinds moved, or a type got its places.
    everything: bool,
    /// The known kinds' revision every node was last looked at with.
    known: Option<u64>,
    /// The types whose module was started because their manifest says they count
    /// (`discovery::counts_unread`) and a node of theirs is held.
    started: HashSet<Uuid>,
    /// How many heads were loaded and counts made, for a test to tell only what moved was.
    #[cfg(test)]
    pub(crate) made: (u64, u64),
}

impl Counting {
    pub(crate) fn set_reads(&mut self, kind: Uuid, layers: Vec<Vec<u8>>) {
        if self
            .reads
            .get(&kind)
            .is_some_and(|(_, held)| *held == layers)
        {
            return;
        }
        self.changes += 1;
        self.reads.insert(kind, (self.changes, layers));
    }

    pub(crate) fn set_all(&mut self, layers: Layers) {
        for (kind, layers) in layers {
            self.set_reads(kind, layers);
        }
    }

    /// The device connected again: what failed, perhaps for being offline, is tried again.
    pub(crate) fn reconnected(&mut self) {
        self.dirty
            .extend(self.failed.drain().map(|(block, _)| block));
    }

    fn forget(&mut self, block: Uuid) {
        self.counted.remove(&block);
        self.heads.remove(&block);
        self.failed.remove(&block);
    }
}

/// The reads kind `content_type` counts from, once its module described it.
fn reads_of(content_type: Uuid) -> Option<Uuid> {
    be_block_type_host::kind_of(content_type)?.reads
}

/// A count to make: of a block's head, loaded, with its place.
struct ToCount {
    block: Uuid,
    content_type: Uuid,
    head: CommitId,
    content: be_sdk_type_api::Loaded,
    place: Option<Vec<u8>>,
}

/// Counts what needs counting: has the modules pick each block's place out of reads that
/// changed, puts each count known for a node's head and place on it, loads the heads not held
/// in a task, and asks the modules to count those held. Whether it changed the graph.
pub(crate) async fn step(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    tasks: &Tasks,
) -> bool {
    let mut moved = pick_places(peer, shared, replicas).await;
    let (to_load, to_count, to_start) = take_dirty(shared);
    let mut started = false;
    for content_type in to_start {
        // Its kind, with the reads kind it counts from, is known once its module described it.
        started |= replicas.kind(content_type).await.is_some();
    }
    if started {
        moved |= pick_places(peer, shared, replicas).await;
    }
    if !to_load.is_empty() {
        let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
        tasks.spawn(async move { load(&peer, &shared, to_load).await });
    }
    for ToCount {
        block,
        content_type,
        head,
        content,
        place,
    } in to_count
    {
        let counted = replicas
            .unread(
                (peer, shared),
                content_type,
                content,
                place.clone(),
                peer.account(),
            )
            .await;
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        #[cfg(test)]
        {
            held.counting.made.1 += 1;
        }
        match counted {
            Ok(count) => {
                held.counting.failed.remove(&block);
                held.counting
                    .counted
                    .insert(block, Counted { head, place, count });
                if held
                    .graph
                    .get(block)
                    .is_some_and(|node| node.head == Some(head))
                {
                    held.graph.set_unread(block, count);
                    held.progressed += 1;
                    moved = true;
                }
            }
            Err(error) => {
                eprintln!("could not count what is unread in {block}: {error}");
                held.counting.failed.insert(block, (head, place));
            }
        }
    }
    moved
}

/// Has each counting type's module pick each block's place out of its reads kind's layers, when
/// they or the known kinds moved: the blocks whose place moved are looked at again, and every
/// node once a type has its first places.
async fn pick_places(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
) -> bool {
    let to_place: Vec<(Uuid, u64, Vec<Vec<u8>>)> = {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let counting = &mut held.counting;
        let look = (counting.changes, be_block_type_host::known_revision());
        if counting.placed == Some(look) {
            return false;
        }
        counting.placed = Some(look);
        be_block_type_host::known()
            .into_iter()
            .filter_map(|(content_type, kind)| {
                let (change, layers) = counting.reads.get(&kind?.reads?)?;
                let current = counting
                    .places
                    .get(&content_type)
                    .is_some_and(|(placed, _)| placed == change);
                (!current).then(|| (content_type, *change, layers.clone()))
            })
            .collect()
    };
    let mut moved = false;
    for (content_type, change, layers) in to_place {
        let places = match replicas.places((peer, shared), content_type, layers).await {
            Ok(places) => places
                .into_iter()
                .map(|place| (place.block, place.place))
                .collect::<HashMap<_, _>>(),
            Err(error) => {
                eprintln!("could not read where the account read {content_type} up to: {error}");
                continue;
            }
        };
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let counting = &mut held.counting;
        match counting.places.insert(content_type, (change, places)) {
            None => counting.everything = true,
            Some((_, before)) => {
                let (_, now) = &counting.places[&content_type];
                let gone = before.keys().filter(|block| !now.contains_key(block));
                let moved_places = now
                    .iter()
                    .filter(|(block, place)| before.get(*block) != Some(*place))
                    .map(|(block, _)| block);
                let dirty: Vec<Uuid> = gone.chain(moved_places).copied().collect();
                counting.dirty.extend(dirty);
            }
        }
        moved = true;
    }
    moved
}

/// Looks at the blocks that moved: a node whose count at its head and place is known takes it,
/// should a listing have put it back without; one whose head is held is counted; one whose head
/// is not is marked as loading. And the types to start the module of: their manifest says they
/// count, and nothing started it yet.
fn take_dirty(
    shared: &Arc<Mutex<Shared>>,
) -> (
    Vec<(Uuid, Uuid, CommitId, Option<Vec<u8>>)>,
    Vec<ToCount>,
    Vec<Uuid>,
) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let state = &mut *held;
    let counting = &mut state.counting;
    let moved = state.seen.graph.take_to_count();
    let known = be_block_type_host::known_revision();
    let everything =
        std::mem::take(&mut counting.everything) || counting.known.replace(known) != Some(known);
    let blocks: Vec<Uuid> = if everything {
        counting.dirty.clear();
        state.seen.graph.nodes().map(|node| node.id).collect()
    } else {
        counting.dirty.drain().chain(moved).collect()
    };
    let mut to_load = Vec::new();
    let mut to_count = Vec::new();
    let mut known_counts = Vec::new();
    let mut to_start = Vec::new();
    for block in blocks {
        let node = state.seen.graph.get(block);
        let Some((node, head)) = node.and_then(|node| Some((node, node.head?))) else {
            counting.forget(block);
            continue;
        };
        if !node.access.can_view() || node.locked {
            counting.forget(block);
            continue;
        }
        if reads_of(node.content_type).is_none() {
            if !counting.started.contains(&node.content_type)
                && crate::discovery::counts_unread(node.content_type)
            {
                counting.started.insert(node.content_type);
                to_start.push(node.content_type);
            }
            continue;
        }
        let Some((_, places)) = counting.places.get(&node.content_type) else {
            continue;
        };
        let place = places.get(&block).cloned();
        match counting.counted.get(&block) {
            Some(counted) if counted.head == head && counted.place == place => {
                if node.unread != counted.count {
                    known_counts.push((block, counted.count));
                }
                continue;
            }
            _ => {}
        }
        if counting.loading.contains(&block)
            || counting.failed.get(&block) == Some(&(head, place.clone()))
        {
            continue;
        }
        match counting.heads.get(&block) {
            Some((at, content)) if *at == head => to_count.push(ToCount {
                block,
                content_type: node.content_type,
                head,
                content: content.clone(),
                place,
            }),
            _ => {
                counting.loading.insert(block);
                to_load.push((block, node.content_type, head, place));
            }
        }
    }
    for (block, count) in known_counts {
        held.graph.set_unread(block, count);
    }
    (to_load, to_count, to_start)
}

/// Loads each head as its type loads it, and holds it for `step` to count.
async fn load(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    heads: Vec<(Uuid, Uuid, CommitId, Option<Vec<u8>>)>,
) {
    for (block, content_type, head, place) in heads {
        let loaded = async {
            let kind = be_block_type_host::kind_of(content_type)
                .ok_or_else(|| format!("no module describes the type {content_type}"))?;
            let commit = peer
                .load_head(block, head)
                .await
                .map_err(|error| error.to_string())?;
            kind.load(peer, &commit.manifest)
                .await
                .map_err(|error| error.to_string())
        }
        .await;
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let counting = &mut held.counting;
        counting.loading.remove(&block);
        #[cfg(test)]
        {
            counting.made.0 += 1;
        }
        match loaded {
            Ok(content) => {
                counting.heads.insert(block, (head, content));
                counting.dirty.insert(block);
            }
            Err(error) => {
                eprintln!("could not load {block} to count what is unread: {error}");
                counting.failed.insert(block, (head, place));
            }
        }
        held.progressed += 1;
    }
    crate::changes::wake();
}
