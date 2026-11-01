use std::collections::{BTreeMap, BTreeSet};

use block_editor_beui::FromEditor;
use block_plugin_api::{
    BlockCommand, EditorInstanceId, Encoded, VersionCommand,
    blocks::{BlockHistory, OpenRequest},
    embed::blocks::Call,
    ops::{Connection, OpError, OpId, Outcome, Producer, encode, wire},
};
use uuid::Uuid;

use crate::ContentStore;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Subject {
    Blocks(block_editor_beui::BlockQuery),
    History(Uuid),
    Head(Uuid),
    Artifact(Uuid),
    Version,
    Shows,
}

struct Watch {
    subject: Subject,
    answering: block_plugin_api::Watch<Encoded>,
    latest: Option<Vec<u8>>,
    next: Option<OpId>,
}

pub(crate) enum Sent {
    Open(OpenRequest),
    Command(Uuid, BlockCommand),
    Version(Uuid, VersionCommand),
    Define(Uuid, Uuid, Option<Vec<u8>>),
    Frame(Uuid, FromEditor),
}

pub(crate) struct Link {
    instance: EditorInstanceId,
    watches: BTreeMap<OpId, Watch>,
    values: BTreeMap<Subject, Vec<u8>>,
    replicas: BTreeMap<OpId, Uuid>,
    producers: BTreeMap<OpId, Producer<Vec<u8>, wire::Bytes>>,
    pub(crate) sent: Vec<Sent>,
}

impl Link {
    pub(crate) fn new(instance: EditorInstanceId) -> Self {
        Self {
            instance,
            watches: BTreeMap::new(),
            values: BTreeMap::new(),
            replicas: BTreeMap::new(),
            producers: BTreeMap::new(),
            sent: Vec::new(),
        }
    }

    pub(crate) fn holds(&self, op: OpId) -> bool {
        self.replicas.contains_key(&op) || self.watches.contains_key(&op)
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
        let mut answering = block_plugin_api::Watch::default();
        answering.opened(op);
        self.watches.insert(
            op,
            Watch {
                subject,
                answering,
                latest,
                next: None,
            },
        );
    }

    pub(crate) fn call(&mut self, call: Call, store: &ContentStore, connection: &mut Connection) {
        let block = Uuid::from_bytes;
        match call {
            Call::OpenBlock { reply, request } => {
                reply.send(connection, &());
                self.sent.push(Sent::Open(request));
            }
            Call::Act {
                reply,
                block: id,
                command,
            } => {
                reply.send(connection, &());
                store.command(block(id), command);
                self.sent.push(Sent::Command(block(id), command));
            }
            Call::Version {
                reply,
                block: id,
                command,
            } => {
                reply.send(connection, &());
                self.sent.push(Sent::Version(block(id), command));
            }
            Call::CreateBlock {
                reply,
                block,
                block_type,
                parent,
                defined,
                content,
            } => {
                reply.send(connection, &());
                store.create_block(block, block_type, parent, defined, content);
            }
            Call::SetParent {
                reply,
                block: id,
                parent,
            } => {
                reply.send(connection, &());
                store.set_parent(block(id), parent);
            }
            Call::Define {
                reply,
                block: id,
                key,
                value,
            } => {
                reply.send(connection, &());
                store.define(block(id), block(key), value.clone());
                self.sent.push(Sent::Define(block(id), block(key), value));
            }
            Call::WriteContent {
                reply,
                block: id,
                content_type,
                bytes,
                replace,
            } => {
                reply.send(connection, &());
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
            Call::ReplicaFrames { this, stream } => {
                self.producers.insert(this.op(), stream);
            }
            Call::ReplicaSend { this, frame, reply } => {
                reply.send(connection, &());
                let Some(id) = self.replicas.get(&this.op()).copied() else {
                    return;
                };
                if let Some(message) = FromEditor::from_frame(&frame) {
                    store.editor_sent(id, self.instance, message.clone());
                    self.sent.push(Sent::Frame(id, message));
                }
            }
            Call::BlocksWatchNew { resource, query } => self.watch(
                resource.op(),
                Subject::Blocks(block_editor_beui::BlockQuery::decode(query)),
            ),
            Call::HistoryWatchNew {
                resource,
                block: id,
            } => {
                self.watch(resource.op(), Subject::History(block(id)));
            }
            Call::HeadWatchNew {
                resource,
                block: id,
            } => {
                self.watch(resource.op(), Subject::Head(block(id)));
            }
            Call::ArtifactWatchNew {
                resource,
                block: id,
            } => {
                self.watch(resource.op(), Subject::Artifact(block(id)));
            }
            Call::VersionWatchNew { resource } => self.watch(resource.op(), Subject::Version),
            Call::ShowRequestsNew { resource } => self.watch(resource.op(), Subject::Shows),
            Call::BlocksWatchNext { this, reply } => self.wait(this.op(), reply.into_op()),
            Call::HistoryWatchNext { this, reply } => self.wait(this.op(), reply.into_op()),
            Call::HeadWatchNext { this, reply } => self.wait(this.op(), reply.into_op()),
            Call::ArtifactWatchNext { this, reply } => self.wait(this.op(), reply.into_op()),
            Call::VersionWatchNext { this, reply } => self.wait(this.op(), reply.into_op()),
            Call::ShowRequestsNext { this, reply } => self.wait(this.op(), reply.into_op()),
        }
    }

    fn wait(&mut self, watch: OpId, next: OpId) {
        if let Some(watch) = self.watches.get_mut(&watch) {
            watch.next = Some(next);
        }
    }

    pub(crate) fn ended(&mut self, op: OpId, store: &ContentStore, connection: &mut Connection) {
        if let Some(block) = self.replicas.remove(&op) {
            self.producers.remove(&op);
            if !self.replicas.values().any(|held| *held == block) {
                store.watch_content(self.instance, block, false);
            }
        } else {
            self.watches.remove(&op);
        }
        connection.complete(op, Outcome::Err(OpError::Canceled));
    }

    pub(crate) fn offer_history(&mut self, block: Uuid, history: &BlockHistory) {
        let value = encode(history);
        self.values.insert(Subject::History(block), value.clone());
        self.offer(Subject::History(block), value);
    }

    pub(crate) fn offer_version(&mut self, status: &block_plugin_api::VersionStatus) {
        let value = encode(status);
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

    pub(crate) fn answer(&mut self, store: &ContentStore, connection: &mut Connection) {
        self.watch_store(store);
        let (replicas, producers) = (&self.replicas, &self.producers);
        let outgoing = store.outgoing(self.instance, |block, frame| {
            let replica = replicas
                .iter()
                .find(|(_, held)| **held == block)
                .map(|(op, _)| *op);
            replica
                .and_then(|op| producers.get(&op))
                .is_none_or(|producer| producer.send(connection, frame).is_ok())
        });
        for (block, head) in outgoing.heads {
            self.offer(Subject::Head(block), encode(&head));
        }
        for (query, listed) in outgoing.listings {
            let listed: Result<Vec<_>, String> = Ok(listed);
            self.offer(Subject::Blocks(query), encode(&listed));
        }
        for watch in self.watches.values_mut() {
            if let Some(next) = watch.next.take() {
                watch.answering.next_encoded(connection, next);
            }
            if let Some(value) = watch.latest.take() {
                watch.answering.set(connection, value);
            }
        }
    }
}

pub(crate) fn history(history: block_editor_beui::BlockHistory) -> BlockHistory {
    BlockHistory {
        head: history.head,
        can_undo: history.can_undo,
        can_redo: history.can_redo,
    }
}
