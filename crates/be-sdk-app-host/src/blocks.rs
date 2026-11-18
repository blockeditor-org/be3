//! The host side of `be3:blocks`: what an editor watches (`Subject`), the watches and replicas it
//! holds (`EditorBlocks`), and the dispatcher (`call`, `ended`, `answers`) that answers its calls over a
//! `BlockStore`. A host keeps its content, its scope and its permissions in its store; the
//! dispatcher keeps the protocol: which call is which, a watch's latest value and what it was last
//! answered, and which replica a frame is sent to. The wire holds each watch's `next` and each
//! replica's stream (`crate::wire::Wire`).

use std::collections::BTreeMap;

use be_sdk_api::{
    BlockCommand, BlockLocation, BlockQuery, EditorInstanceId, MetadataEntries, SearchResults,
    blocks::{BlockSubject, BlockValue, OpenRequest, ShowRequest},
    embed::blocks::Call,
    ops::{OpError, OpId, Outcome, Serialize, encode},
};
use uuid::Uuid;

/// What a block watch follows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Subject {
    Blocks(BlockQuery),
    History(Uuid),
    Head(Uuid),
    Artifact(Uuid),
    Version,
    /// The blocks, histories and panels the host asks the editor to show, a queue the store
    /// hands over when the watch waits (`BlockStore::take_shows`).
    Shows,
    /// A full-text search, named by its watch: the store holds what it asks
    /// (`BlockStore::search_opened`).
    Search(OpId),
}

impl From<BlockSubject> for Subject {
    fn from(subject: BlockSubject) -> Self {
        match subject {
            BlockSubject::Blocks(query) => Self::Blocks(query),
            BlockSubject::History(block) => Self::History(Uuid::from_bytes(block)),
            BlockSubject::Head(block) => Self::Head(Uuid::from_bytes(block)),
            BlockSubject::Artifact(block) => Self::Artifact(Uuid::from_bytes(block)),
            BlockSubject::Version => Self::Version,
        }
    }
}

impl Subject {
    /// Whether `value` is of this subject's variant.
    pub fn answered_by(self, value: &BlockValue) -> bool {
        matches!(
            (self, value),
            (Self::Blocks(_), BlockValue::Blocks(_))
                | (Self::History(_), BlockValue::History(_))
                | (Self::Head(_), BlockValue::Head(_))
                | (Self::Artifact(_), BlockValue::Artifact(_))
                | (Self::Version, BlockValue::Version(_))
        )
    }
}

struct Watch {
    subject: Subject,
    /// What the editor was last answered, which an offer of the same value does not repeat.
    answered: Option<Vec<u8>>,
    /// A value it has not been answered yet, until it waits for the next.
    latest: Option<Vec<u8>>,
}

impl Watch {
    fn offer(&mut self, bytes: &[u8]) {
        self.latest = (self.answered.as_deref() != Some(bytes)).then(|| bytes.to_vec());
    }

    fn take(&mut self) -> Option<Vec<u8>> {
        let bytes = self.latest.take()?;
        self.answered = Some(bytes.clone());
        Some(bytes)
    }
}

/// The block watches and replicas one editor holds open, by op. Only the dispatcher adds to or
/// removes from it; a store offers its watches values.
#[derive(Default)]
pub struct EditorBlocks {
    watches: BTreeMap<OpId, Watch>,
    /// Each replica's block and content type.
    replicas: BTreeMap<OpId, (Uuid, Uuid)>,
}

impl EditorBlocks {
    pub fn watching(&self) -> impl Iterator<Item = Subject> + '_ {
        self.watches.values().map(|watch| watch.subject)
    }

    /// Each watch, by its op, with what it follows.
    pub fn watches(&self) -> impl Iterator<Item = (OpId, Subject)> + '_ {
        self.watches.iter().map(|(op, watch)| (*op, watch.subject))
    }

    /// Offers the watches of `subject` its latest value, a `BlockValue` of the variant of the
    /// same name. A watch already answered that value is not answered it again.
    pub fn offer(&mut self, subject: Subject, value: &BlockValue) {
        debug_assert!(subject.answered_by(value), "{subject:?} offered {value:?}");
        self.offer_encoded(subject, value);
    }

    /// Offers the search watch `op` its latest results.
    pub fn offer_search(&mut self, op: OpId, results: &SearchResults) {
        self.offer_encoded(Subject::Search(op), results);
    }

    fn offer_encoded(&mut self, subject: Subject, value: &impl Serialize) {
        let bytes = encode(value);
        for watch in self.watches.values_mut() {
            if watch.subject == subject {
                watch.offer(&bytes);
            }
        }
    }

    /// Whether `op` is a watch or replica it holds.
    pub fn holds(&self, op: OpId) -> bool {
        self.watches.contains_key(&op) || self.replicas.contains_key(&op)
    }

    /// The replica frames for `block` are relayed to: the latest it opened. A host holds one
    /// participant per editor and block, so an earlier replica of the same block hears nothing
    /// until the later ones end, and is then brought up to date (`BlockStore::replica_resumed`).
    pub fn replica(&self, block: Uuid) -> Option<OpId> {
        self.replicas
            .iter()
            .rev()
            .find(|(_, (held, _))| *held == block)
            .map(|(op, _)| *op)
    }

    /// Forgets every watch and replica, as an app that restarts holds none.
    pub fn clear(&mut self) {
        self.watches.clear();
        self.replicas.clear();
    }
}

/// A block an editor makes (`blocks.create-block`).
pub struct Created {
    pub block: Uuid,
    pub block_type: Uuid,
    pub parent: BlockLocation,
    pub defined: MetadataEntries,
    pub content: Option<Vec<u8>>,
    pub owner_only: bool,
}

/// Content an editor writes to a block (`blocks.write-content`).
pub struct Written {
    pub block: Uuid,
    pub content_type: Uuid,
    pub bytes: Vec<u8>,
    pub replace: bool,
}

/// What a host's blocks are to the dispatcher. Each call that changes something returns whether
/// it changed what the host shows (`Ok(true)`), or the error the editor is answered.
pub trait BlockStore {
    /// What `instance` holds, or `None` for an instance the store does not know, whose calls go
    /// unanswered here. The store keeps it where it likes, but only the dispatcher adds watches
    /// and replicas to it or removes them.
    fn held(&mut self, instance: EditorInstanceId) -> Option<&mut EditorBlocks>;

    /// The instance holding the watch or replica `op`: the one whose `held` has it. The store
    /// may answer from a map of its own, kept as the dispatcher opens and ends them.
    fn holder(&self, op: OpId) -> Option<EditorInstanceId>;

    /// The host's policy before a call is dispatched: it may translate the block ids the call
    /// carries, or refuse it.
    fn admit(&mut self, _instance: EditorInstanceId, _call: &mut Call) -> Result<(), OpError> {
        Ok(())
    }

    fn open_block(
        &mut self,
        instance: EditorInstanceId,
        request: OpenRequest,
    ) -> Result<bool, OpError>;

    fn act(
        &mut self,
        instance: EditorInstanceId,
        block: Uuid,
        command: BlockCommand,
    ) -> Result<bool, OpError>;

    fn create_block(
        &mut self,
        instance: EditorInstanceId,
        created: Created,
    ) -> Result<bool, OpError>;

    fn write_content(
        &mut self,
        instance: EditorInstanceId,
        written: Written,
    ) -> Result<bool, OpError>;

    /// `instance` opened the replica `op` of `block`'s content, `content_type`.
    fn replica_opened(
        &mut self,
        instance: EditorInstanceId,
        op: OpId,
        block: Uuid,
        content_type: Uuid,
    ) -> bool;

    /// A frame `instance` sent its replica of `block`.
    fn replica_sent(&mut self, instance: EditorInstanceId, block: Uuid, frame: Vec<u8>) -> bool;

    /// The latest replica of `block` ended while `instance` holds the earlier `op`, of
    /// `content_type`, which heard nothing since that one opened. The store drops what it queued
    /// for the replica that ended and starts `op` afresh: a snapshot, then what follows it.
    fn replica_resumed(
        &mut self,
        instance: EditorInstanceId,
        op: OpId,
        block: Uuid,
        content_type: Uuid,
    );

    /// `instance` holds no replica of `block` any more.
    fn replica_ended(&mut self, instance: EditorInstanceId, block: Uuid);

    /// `instance` opened the watch `op` of `subject`, which is in its `EditorBlocks` already.
    fn watch_opened(&mut self, instance: EditorInstanceId, op: OpId, subject: Subject) -> bool;

    /// `instance` opened the search watch `op`, of `text` within `within` or everywhere.
    fn search_opened(
        &mut self,
        instance: EditorInstanceId,
        op: OpId,
        _text: String,
        _within: Option<Uuid>,
    ) -> bool {
        self.watch_opened(instance, op, Subject::Search(op))
    }

    /// `instance` dropped a watch of `subject`, which is gone from its `EditorBlocks`.
    fn watch_ended(&mut self, _instance: EditorInstanceId, _subject: Subject) {}

    /// What the host asks `instance` to show since its shows watch was last answered.
    fn take_shows(&mut self, _instance: EditorInstanceId) -> Vec<ShowRequest> {
        Vec::new()
    }
}

/// How a call is answered at once.
#[derive(Debug, PartialEq)]
pub enum Reply {
    Answer(OpId, Outcome),
    /// A frame sent to a replica, taken in (`Wire::accepted`).
    Accepted(OpId),
}

#[derive(Debug, Default, PartialEq)]
pub struct Dispatched {
    pub reply: Option<Reply>,
    /// Whether it changed what the host shows.
    pub changed: bool,
}

/// Dispatches one `be3:blocks` call `instance` made (`Heard::Blocks`) to `store`.
pub fn call(store: &mut impl BlockStore, instance: EditorInstanceId, mut call: Call) -> Dispatched {
    if let Err(error) = store.admit(instance, &mut call) {
        let (op, outcome) = call.refuse(error);
        return Dispatched {
            reply: Some(Reply::Answer(op, outcome)),
            changed: false,
        };
    }
    let done = |reply: be_sdk_api::ops::Reply<()>, result: Result<bool, OpError>| match result {
        Ok(changed) => {
            let (op, outcome) = reply.answer(&());
            Dispatched {
                reply: Some(Reply::Answer(op, outcome)),
                changed,
            }
        }
        Err(error) => Dispatched {
            reply: Some(Reply::Answer(reply.into_op(), Outcome::Err(error))),
            changed: false,
        },
    };
    let changed = |changed| Dispatched {
        reply: None,
        changed,
    };
    match call {
        Call::OpenBlock { reply, request } => done(reply, store.open_block(instance, request)),
        Call::Act {
            reply,
            block,
            command,
        } => done(reply, store.act(instance, Uuid::from_bytes(block), command)),
        Call::WriteContent {
            reply,
            block,
            content_type,
            bytes,
            replace,
        } => done(
            reply,
            store.write_content(
                instance,
                Written {
                    block: Uuid::from_bytes(block),
                    content_type: Uuid::from_bytes(content_type),
                    bytes,
                    replace,
                },
            ),
        ),
        Call::CreateBlock {
            reply,
            block,
            block_type,
            parent,
            defined,
            content,
            owner_only,
        } => done(
            reply,
            store.create_block(
                instance,
                Created {
                    block: Uuid::from_bytes(block),
                    block_type: Uuid::from_bytes(block_type),
                    parent,
                    defined,
                    content,
                    owner_only,
                },
            ),
        ),
        Call::ReplicaNew {
            resource,
            block,
            content_type,
        } => {
            let op = resource.op();
            let block = Uuid::from_bytes(block);
            let Some(held) = store.held(instance) else {
                return changed(false);
            };
            let content_type = Uuid::from_bytes(content_type);
            held.replicas.insert(op, (block, content_type));
            changed(store.replica_opened(instance, op, block, content_type))
        }
        Call::ReplicaSend { reply, this, frame } => {
            let held = store.held(instance);
            let block = held
                .as_ref()
                .and_then(|held| held.replicas.get(&this.op()).map(|(block, _)| *block));
            // The host holds one participant per editor and block, which the latest replica
            // speaks for: an earlier one may not send.
            if let Some(block) = block
                && held.is_some_and(|held| held.replica(block) != Some(this.op()))
            {
                return Dispatched {
                    reply: Some(Reply::Answer(
                        reply.into_op(),
                        Outcome::Err(OpError::Denied),
                    )),
                    changed: false,
                };
            }
            Dispatched {
                reply: Some(Reply::Accepted(reply.into_op())),
                changed: block.is_some_and(|block| store.replica_sent(instance, block, frame)),
            }
        }
        Call::BlockWatchNew { resource, subject } => {
            let subject = Subject::from(subject);
            changed(
                watch(store, instance, resource.op(), subject)
                    .is_some_and(|op| store.watch_opened(instance, op, subject)),
            )
        }
        Call::SearchWatchNew {
            resource,
            text,
            within,
        } => {
            let op = resource.op();
            changed(
                watch(store, instance, op, Subject::Search(op)).is_some_and(|op| {
                    store.search_opened(instance, op, text, within.map(Uuid::from_bytes))
                }),
            )
        }
        Call::ShowRequestsNew { resource } => changed(
            watch(store, instance, resource.op(), Subject::Shows)
                .is_some_and(|op| store.watch_opened(instance, op, Subject::Shows)),
        ),
        // The wire holds a replica's stream and each watch's `next` itself.
        call @ (Call::ReplicaFrames { .. }
        | Call::BlockWatchNext { .. }
        | Call::SearchWatchNext { .. }
        | Call::ShowRequestsNext { .. }) => {
            let (op, outcome) = call.refuse(OpError::Unsupported);
            Dispatched {
                reply: Some(Reply::Answer(op, outcome)),
                changed: false,
            }
        }
    }
}

fn watch(
    store: &mut impl BlockStore,
    instance: EditorInstanceId,
    op: OpId,
    subject: Subject,
) -> Option<OpId> {
    store.held(instance)?.watches.insert(
        op,
        Watch {
            subject,
            answered: None,
            latest: None,
        },
    );
    Some(op)
}

/// Forgets the watch or replica `op` the editor dropped, which its wire has answered
/// (`Heard::Ended`). Whether it was one.
pub fn ended(store: &mut impl BlockStore, op: OpId) -> bool {
    let Some(instance) = store.holder(op) else {
        return false;
    };
    let Some(held) = store.held(instance) else {
        return false;
    };
    if let Some(watch) = held.watches.remove(&op) {
        store.watch_ended(instance, watch.subject);
        return true;
    }
    let Some((block, _)) = held.replicas.remove(&op) else {
        return false;
    };
    match held.replica(block) {
        None => store.replica_ended(instance, block),
        // `op` was the latest, which the frames went to.
        Some(earlier) if earlier < op => {
            let (_, content_type) = held.replicas[&earlier];
            store.replica_resumed(instance, earlier, block, content_type);
        }
        Some(_) => {}
    }
    true
}

/// The answers to the watches in `waiting` (`Wire::waiting`) that have a value they were not
/// answered yet, for `Wire::answer_watch`.
pub fn answers(store: &mut impl BlockStore, waiting: &[OpId]) -> Vec<(OpId, Vec<u8>)> {
    let mut answers = Vec::new();
    for op in waiting {
        let Some(instance) = store.holder(*op) else {
            continue;
        };
        let Some(watch) = store
            .held(instance)
            .and_then(|held| held.watches.get_mut(op))
        else {
            continue;
        };
        if watch.subject != Subject::Shows {
            answers.extend(watch.take().map(|bytes| (*op, bytes)));
            continue;
        }
        let shows = store.take_shows(instance);
        if !shows.is_empty() {
            answers.push((*op, encode(&shows)));
        }
    }
    answers
}

#[cfg(test)]
mod tests;
