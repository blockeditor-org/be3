use std::collections::{BTreeMap, BTreeSet};

use be_sdk_api::{
    BlockCommand, EditorInstanceId,
    blocks::{BlockHistory, BlockSubject, BlockValue, OpenRequest},
    embed::blocks::Call,
    ops::{OpId, Reply, encode},
};
use be_sdk_app_host::wire::Wire;
use be_sdk_beui::FromEditor;
use uuid::Uuid;

use crate::ContentStore;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Subject {
    Blocks(be_sdk_beui::BlockQuery),
    History(Uuid),
    Head(Uuid),
    Artifact(Uuid),
    Version,
    Shows,
    /// A full-text search, which this store never answers: it keeps no index.
    Search,
}

struct Watch {
    subject: Subject,
    /// Its latest value, until the editor asks for the next.
    latest: Option<Vec<u8>>,
}

pub(crate) enum Sent {
    Open(OpenRequest),
    Command(Uuid, BlockCommand),
    Define(Uuid, Uuid, Option<Vec<u8>>),
    Frame(Uuid, FromEditor),
}

/// The editor's block calls, answered from the test's `ContentStore` through the editor's wire,
/// which holds its watches' `next`s, its replicas' streams and its sends (`Wire`).
pub(crate) struct Link {
    instance: EditorInstanceId,
    watches: BTreeMap<OpId, Watch>,
    values: BTreeMap<Subject, Vec<u8>>,
    replicas: BTreeMap<OpId, Uuid>,
    pub(crate) sent: Vec<Sent>,
}

impl Link {
    pub(crate) fn new(instance: EditorInstanceId) -> Self {
        Self {
            instance,
            watches: BTreeMap::new(),
            values: BTreeMap::new(),
            replicas: BTreeMap::new(),
            sent: Vec::new(),
        }
    }

    fn watch_store(&self, store: &ContentStore) {
        let subjects = || self.watches.values().map(|watch| watch.subject);
        store.watch_heads(
            subjects()
                .filter_map(|subject| match subject {
                    Subject::Head(block) => Some(block),
                    _ => None,
                })
                .collect::<BTreeSet<_>>(),
        );
        let mut queries: Vec<_> = subjects()
            .filter_map(|subject| match subject {
                Subject::Blocks(query) => Some(query),
                _ => None,
            })
            .collect();
        queries.sort();
        queries.dedup();
        store.watch_queries(queries);
    }

    fn watch(&mut self, op: OpId, subject: Subject) {
        let latest = self.values.get(&subject).cloned();
        self.watches.insert(op, Watch { subject, latest });
    }

    /// `author` is the account the editor's host is signed in to, which the blocks it makes
    /// are by. A call that changes something is answered along with the next thing the test
    /// sends, as an acknowledgement (`Wire::acknowledge`): its answer alone steps the editor for
    /// nothing.
    pub(crate) fn call(&mut self, call: Call, store: &ContentStore, wire: &mut Wire, author: Uuid) {
        let block = Uuid::from_bytes;
        let done = |wire: &mut Wire, reply: Reply<()>| {
            let (op, outcome) = reply.answer(&());
            wire.acknowledge(op, outcome);
        };
        match call {
            Call::OpenBlock { reply, request } => {
                done(wire, reply);
                self.sent.push(Sent::Open(request));
            }
            Call::Act {
                reply,
                block: id,
                command,
            } => {
                done(wire, reply);
                let id = block(id);
                match command {
                    BlockCommand::SetParent(parent) => store.set_parent(id, parent),
                    BlockCommand::Define { key, value } => {
                        store.define(id, block(key), value.clone());
                        self.sent.push(Sent::Define(id, block(key), value));
                    }
                    command => {
                        store.command(id, &command);
                        self.sent.push(Sent::Command(id, command));
                    }
                }
            }
            Call::CreateBlock {
                reply,
                block,
                block_type,
                parent,
                defined,
                content,
                owner_only,
            } => {
                done(wire, reply);
                store.create_block(
                    block, block_type, parent, defined, content, owner_only, author,
                );
            }
            Call::WriteContent {
                reply,
                block: id,
                content_type,
                bytes,
                replace,
            } => {
                done(wire, reply);
                store.seed_content(block(id), block(content_type), bytes, replace);
            }
            Call::ReplicaNew {
                resource,
                block: id,
                ..
            } => {
                self.replicas.insert(resource.op(), block(id));
                store.watch_content(self.instance, block(id), true);
            }
            Call::ReplicaSend { this, frame, reply } => {
                wire.accepted(reply.into_op());
                let Some(id) = self.replicas.get(&this.op()).copied() else {
                    return;
                };
                if let Some(message) = FromEditor::from_frame(&frame) {
                    store.editor_sent(id, self.instance, message.clone());
                    self.sent.push(Sent::Frame(id, message));
                }
            }
            Call::BlockWatchNew { resource, subject } => {
                let subject = match subject {
                    BlockSubject::Blocks(query) => {
                        Subject::Blocks(be_sdk_beui::BlockQuery::decode(query))
                    }
                    BlockSubject::History(id) => Subject::History(block(id)),
                    BlockSubject::Head(id) => Subject::Head(block(id)),
                    BlockSubject::Artifact(id) => Subject::Artifact(block(id)),
                    BlockSubject::Version => Subject::Version,
                };
                self.watch(resource.op(), subject);
            }
            Call::SearchWatchNew { resource, .. } => self.watch(resource.op(), Subject::Search),
            Call::ShowRequestsNew { resource } => self.watch(resource.op(), Subject::Shows),
            // The wire holds a replica's stream and each watch's `next` itself.
            call @ (Call::ReplicaFrames { .. }
            | Call::BlockWatchNext { .. }
            | Call::SearchWatchNext { .. }
            | Call::ShowRequestsNext { .. }) => {
                let (op, outcome) = call.refuse(be_sdk_api::ops::OpError::Unsupported);
                wire.answer(op, outcome);
            }
        }
    }

    /// A watch or replica the editor dropped, which its wire has answered.
    pub(crate) fn ended(&mut self, op: OpId, store: &ContentStore) {
        if let Some(block) = self.replicas.remove(&op) {
            if !self.replicas.values().any(|held| *held == block) {
                store.watch_content(self.instance, block, false);
            }
        } else {
            self.watches.remove(&op);
        }
    }

    pub(crate) fn offer_history(&mut self, block: Uuid, history: &BlockHistory) {
        let value = encode(&BlockValue::History(*history));
        self.values.insert(Subject::History(block), value.clone());
        self.offer(Subject::History(block), value);
    }

    pub(crate) fn offer_artifact(&mut self, block: Uuid, state: Option<be_sdk_api::ArtifactState>) {
        let value = encode(&BlockValue::Artifact(state));
        self.values.insert(Subject::Artifact(block), value.clone());
        self.offer(Subject::Artifact(block), value);
    }

    /// Each watch the editor holds open, by its op, with what it watches.
    #[cfg(test)]
    pub(crate) fn watching(&self) -> Vec<(OpId, Subject)> {
        self.watches
            .iter()
            .map(|(op, watch)| (*op, watch.subject))
            .collect()
    }

    pub(crate) fn offer_version(&mut self, status: &be_sdk_api::VersionStatus) {
        let value = encode(&BlockValue::Version(status.clone()));
        self.values.insert(Subject::Version, value.clone());
        self.offer(Subject::Version, value);
    }

    fn offer(&mut self, subject: Subject, value: Vec<u8>) {
        for watch in self.watches.values_mut() {
            if watch.subject == subject {
                watch.latest = Some(value.clone());
            }
        }
    }

    /// Relays what the store has for the editor's replicas, and answers each watch the editor
    /// waits on that has a new value.
    pub(crate) fn answer(&mut self, store: &ContentStore, wire: &mut Wire) {
        self.watch_store(store);
        let replicas = &self.replicas;
        let outgoing = store.outgoing(self.instance, |block, frame| {
            replicas
                .iter()
                .find(|(_, held)| **held == block)
                .is_none_or(|(op, _)| wire.relay(*op, frame))
        });
        for (block, head) in outgoing.heads {
            self.offer(
                Subject::Head(block),
                encode(&BlockValue::Head(head.map_err(be_sdk_api::Failure::Failed))),
            );
        }
        for (query, listed) in outgoing.listings {
            self.offer(
                Subject::Blocks(query),
                encode(&BlockValue::Blocks(Ok(listed))),
            );
        }
        for op in wire.waiting() {
            if let Some(value) = self
                .watches
                .get_mut(&op)
                .and_then(|watch| watch.latest.take())
            {
                wire.answer_watch(op, value);
            }
        }
    }
}

pub(crate) fn history(history: be_sdk_beui::BlockHistory) -> BlockHistory {
    BlockHistory {
        head: history.head,
        can_undo: history.can_undo,
        can_redo: history.can_redo,
        restarted_at: history.restarted_at,
        lost_edits: history.lost_edits,
    }
}
