//! Unread counts on the graph's nodes (guides/blocks/host.md, "Unread counts"): for a block
//! whose type counts what the account has not read (`type-info.reads`,
//! `be_block_content::Unread`), the worker loads its head as the type loads it (a chat: only its
//! header) and asks its module, with the account's layers of the type's reads kind, which the
//! page reads from the account's settings and sends here (`publish`). The count lands on the
//! node (`Node::unread`), and so on every row an app draws of the block (`BlockInfo::unread`).
//! No replica is opened for it: one load per head, and one call per head or change of reads.

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

/// A count made: at which head, with which reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Counted {
    head: CommitId,
    reads: u64,
    count: u32,
}

/// The worker's part, in `Shared`: the reads the page sent and what was counted with them.
#[derive(Default)]
pub(crate) struct Counting {
    /// Each reads kind's layers, and the number of the change that set them.
    reads: HashMap<Uuid, (u64, Vec<Vec<u8>>)>,
    changes: u64,
    counted: HashMap<Uuid, Counted>,
    /// Heads being loaded, and those loaded waiting for their module.
    loading: HashSet<Uuid>,
    loaded: Vec<(Uuid, Uuid, CommitId, be_sdk_type_api::Loaded)>,
    /// Heads that could not be counted (no key, offline, a type that failed): not tried again
    /// until the head or the reads change.
    failed: HashMap<Uuid, (CommitId, u64)>,
    /// The graph's revision, the reads' and the known kinds' last looked at, so a pass with
    /// nothing new scans no node.
    looked: Option<(u64, u64, u64)>,
    /// The types whose module was started because their manifest says they count
    /// (`discovery::counts_unread`) and a node of theirs is held.
    started: HashSet<Uuid>,
}

impl Counting {
    pub(crate) fn set_reads(&mut self, kind: Uuid, layers: Vec<Vec<u8>>) {
        if self.reads.get(&kind).is_some_and(|(_, held)| *held == layers) {
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
}

/// The reads kind `content_type` counts from, once its module described it.
fn reads_of(content_type: Uuid) -> Option<Uuid> {
    be_block_type_host::kind_of(content_type)?.reads
}

/// Counts what needs counting: puts each count known for a node's head and reads on it, loads
/// the heads not counted yet in a task, and asks the modules about those loaded. Whether it
/// changed the graph.
pub(crate) async fn step(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    replicas: &mut Replicas,
    tasks: &Tasks,
) -> bool {
    let (to_load, to_start) = take_to_load(shared);
    let mut moved = false;
    for content_type in to_start {
        // Its kind, with the reads kind it counts from, is known once its module described it.
        moved |= replicas.kind(content_type).await.is_some();
    }
    if moved {
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .counting
            .looked = None;
    }
    if !to_load.is_empty() {
        let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
        tasks.spawn(async move { load(&peer, &shared, to_load).await });
    }
    let loaded = std::mem::take(
        &mut shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .counting
            .loaded,
    );
    for (block, content_type, head, content) in loaded {
        let Some((reads, layers)) = reads_of(content_type).and_then(|kind| {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .counting
                .reads
                .get(&kind)
                .cloned()
        }) else {
            continue;
        };
        let counted = replicas
            .unread(
                (peer, shared),
                content_type,
                content,
                layers,
                block,
                peer.account(),
            )
            .await;
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.counting.loading.remove(&block);
        match counted {
            Ok(count) => {
                held.counting
                    .counted
                    .insert(block, Counted { head, reads, count });
                held.counting.failed.remove(&block);
                if held.graph.get(block).is_some_and(|node| node.head == Some(head)) {
                    held.graph.set_unread(block, count);
                    held.progressed += 1;
                    moved = true;
                }
            }
            Err(error) => {
                eprintln!("could not count what is unread in {block}: {error}");
                held.counting.failed.insert(block, (head, reads));
            }
        }
        held.counting.looked = None;
    }
    moved
}

/// The nodes whose count at their head, with the reads now, is not known: each marked as
/// loading. A node whose count is known takes it, should a listing have put it back without.
/// And the types to start the module of: their manifest says they count, and nothing started
/// it yet.
fn take_to_load(shared: &Arc<Mutex<Shared>>) -> (Vec<(Uuid, Uuid, CommitId)>, Vec<Uuid>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let look = (
        held.graph.revision,
        held.counting.changes,
        be_block_type_host::known_revision(),
    );
    if held.counting.looked == Some(look) {
        return (Vec::new(), Vec::new());
    }
    held.counting.looked = Some(look);
    let state = &mut *held;
    let counting = &mut state.counting;
    let mut to_load = Vec::new();
    let mut known = Vec::new();
    let mut present = HashSet::new();
    let mut to_start = Vec::new();
    for node in state.seen.graph.nodes() {
        let Some(head) = node.head else {
            continue;
        };
        if !node.access.can_view() || node.locked {
            continue;
        }
        let Some(kind) = reads_of(node.content_type) else {
            if !counting.started.contains(&node.content_type)
                && crate::discovery::counts_unread(node.content_type)
            {
                counting.started.insert(node.content_type);
                to_start.push(node.content_type);
            }
            continue;
        };
        let Some(&(reads, _)) = counting.reads.get(&kind) else {
            continue;
        };
        present.insert(node.id);
        match counting.counted.get(&node.id) {
            Some(counted) if counted.head == head && counted.reads == reads => {
                if node.unread != counted.count {
                    known.push((node.id, counted.count));
                }
                continue;
            }
            _ => {}
        }
        if counting.loading.contains(&node.id)
            || counting.failed.get(&node.id) == Some(&(head, reads))
        {
            continue;
        }
        counting.loading.insert(node.id);
        to_load.push((node.id, node.content_type, head));
    }
    counting.counted.retain(|block, _| present.contains(block));
    counting.failed.retain(|block, _| present.contains(block));
    for (block, count) in known {
        held.graph.set_unread(block, count);
    }
    (to_load, to_start)
}

/// Loads each head as its type loads it, for `step` to count.
async fn load(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, heads: Vec<(Uuid, Uuid, CommitId)>) {
    for (block, content_type, head) in heads {
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
        match loaded {
            Ok(content) => held
                .counting
                .loaded
                .push((block, content_type, head, content)),
            Err(error) => {
                held.counting.loading.remove(&block);
                let reads = reads_of(content_type)
                    .and_then(|kind| held.counting.reads.get(&kind))
                    .map_or(0, |(reads, _)| *reads);
                eprintln!("could not load {block} to count what is unread: {error}");
                held.counting.failed.insert(block, (head, reads));
                held.counting.looked = None;
            }
        }
        held.progressed += 1;
    }
    crate::changes::wake();
}
