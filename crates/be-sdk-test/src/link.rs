use std::collections::BTreeSet;

use be_sdk_api::{
    BlockCommand, EditorInstanceId,
    blocks::{BlockHistory, BlockValue, OpenRequest},
    embed::blocks::Call,
    ops::{OpError, OpId},
};
use be_sdk_app_host::{
    blocks::{self, BlockStore, Created, EditorBlocks, Reply, Subject, Written},
    wire::Wire,
};
use be_sdk_beui::FromEditor;
use uuid::Uuid;

use crate::ContentStore;

pub(crate) enum Sent {
    Open(OpenRequest),
    Command(Uuid, BlockCommand),
    Define(Uuid, Uuid, Option<Vec<u8>>),
    Frame(Uuid, FromEditor),
}

/// The editor's block calls, answered from the test's `ContentStore` by the library's dispatcher
/// (`be_sdk_app_host::blocks`) through the editor's wire, which holds its watches' `next`s, its
/// replicas' streams and its sends (`Wire`).
pub(crate) struct Link {
    instance: EditorInstanceId,
    held: EditorBlocks,
    /// The latest value the test set of each subject it sets, which a watch opened later starts
    /// with.
    values: Vec<(Subject, BlockValue)>,
    pub(crate) sent: Vec<Sent>,
}

/// The `Link` with what its calls reach: the store, and the account the editor's host is signed
/// in to, which the blocks it makes are by: `None` where no call is dispatched (`ended`,
/// `answers`), which refuses one that makes a block.
struct Linked<'a> {
    link: &'a mut Link,
    store: &'a ContentStore,
    author: Option<Uuid>,
}

impl BlockStore for Linked<'_> {
    fn held(&mut self, instance: EditorInstanceId) -> Option<&mut EditorBlocks> {
        (instance == self.link.instance).then_some(&mut self.link.held)
    }

    fn holder(&self, op: OpId) -> Option<EditorInstanceId> {
        self.link.held.holds(op).then_some(self.link.instance)
    }

    fn open_block(&mut self, _: EditorInstanceId, request: OpenRequest) -> Result<bool, OpError> {
        self.link.sent.push(Sent::Open(request));
        Ok(true)
    }

    fn act(
        &mut self,
        _: EditorInstanceId,
        block: Uuid,
        command: BlockCommand,
    ) -> Result<bool, OpError> {
        match command {
            BlockCommand::SetParent(parent) => self.store.set_parent(block, parent),
            BlockCommand::Define { key, value } => {
                let key = Uuid::from_bytes(key);
                self.store.define(block, key, value.clone());
                self.link.sent.push(Sent::Define(block, key, value));
            }
            command => {
                self.store.command(block, &command);
                self.link.sent.push(Sent::Command(block, command));
            }
        }
        Ok(true)
    }

    fn create_block(&mut self, _: EditorInstanceId, created: Created) -> Result<bool, OpError> {
        let author = self.author.ok_or(OpError::Denied)?;
        self.store.create_block(
            created.block.into_bytes(),
            created.block_type.into_bytes(),
            created.parent,
            created.defined,
            created.content,
            created.owner_only,
            author,
        );
        Ok(true)
    }

    fn write_content(&mut self, _: EditorInstanceId, written: Written) -> Result<bool, OpError> {
        self.store.seed_content(
            written.block,
            written.content_type,
            written.bytes,
            written.replace,
        );
        Ok(true)
    }

    fn replica_opened(&mut self, instance: EditorInstanceId, _: OpId, block: Uuid, _: Uuid) -> bool {
        self.store.watch_content(instance, block, true);
        true
    }

    fn replica_sent(&mut self, instance: EditorInstanceId, block: Uuid, frame: Vec<u8>) -> bool {
        let Some(message) = FromEditor::from_frame(&frame) else {
            return false;
        };
        self.store.editor_sent(block, instance, message.clone());
        self.link.sent.push(Sent::Frame(block, message));
        true
    }

    fn replica_resumed(&mut self, instance: EditorInstanceId, _: OpId, block: Uuid, _: Uuid) {
        self.store.resume_content(instance, block);
    }

    fn replica_ended(&mut self, instance: EditorInstanceId, block: Uuid) {
        self.store.watch_content(instance, block, false);
    }

    fn watch_opened(&mut self, _: EditorInstanceId, _: OpId, subject: Subject) -> bool {
        if let Some((_, value)) = self.link.values.iter().find(|(set, _)| *set == subject) {
            self.link.held.offer(subject, value);
        }
        true
    }
}

impl Link {
    pub(crate) fn new(instance: EditorInstanceId) -> Self {
        Self {
            instance,
            held: EditorBlocks::default(),
            values: Vec::new(),
            sent: Vec::new(),
        }
    }

    fn linked<'a>(&'a mut self, store: &'a ContentStore, author: Option<Uuid>) -> Linked<'a> {
        Linked {
            link: self,
            store,
            author,
        }
    }

    fn watch_store(&self, store: &ContentStore) {
        let subjects = || self.held.watching();
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
                Subject::Blocks(query) => Some(be_sdk_beui::BlockQuery::decode(query)),
                _ => None,
            })
            .collect();
        queries.sort();
        queries.dedup();
        store.watch_queries(queries);
    }

    /// `author` is the account the editor's host is signed in to, which the blocks it makes
    /// are by. Each call is answered at once, as be-host's runtime answers it.
    pub(crate) fn call(&mut self, call: Call, store: &ContentStore, wire: &mut Wire, author: Uuid) {
        let instance = self.instance;
        match blocks::call(&mut self.linked(store, Some(author)), instance, call).reply {
            Some(Reply::Answer(op, outcome)) => wire.answer(op, outcome),
            Some(Reply::Accepted(op)) => wire.accepted(op),
            None => {}
        }
    }

    /// A watch or replica the editor dropped, which its wire has answered.
    pub(crate) fn ended(&mut self, op: OpId, store: &ContentStore) {
        blocks::ended(&mut self.linked(store, None), op);
    }

    pub(crate) fn offer_history(&mut self, block: Uuid, history: &BlockHistory) {
        self.set(Subject::History(block), BlockValue::History(*history));
    }

    pub(crate) fn offer_artifact(&mut self, block: Uuid, state: Option<be_sdk_api::ArtifactState>) {
        self.set(Subject::Artifact(block), BlockValue::Artifact(state));
    }

    pub(crate) fn offer_version(&mut self, status: &be_sdk_api::VersionStatus) {
        self.set(Subject::Version, BlockValue::Version(status.clone()));
    }

    fn set(&mut self, subject: Subject, value: BlockValue) {
        self.held.offer(subject, &value);
        self.values.retain(|(set, _)| *set != subject);
        self.values.push((subject, value));
    }

    /// Each watch the editor holds open, by its op, with what it watches.
    #[cfg(test)]
    pub(crate) fn watching(&self) -> Vec<(OpId, Subject)> {
        self.held.watches().collect()
    }

    /// Relays what the store has for the editor's replicas, and answers each watch the editor
    /// waits on that has a new value.
    pub(crate) fn answer(&mut self, store: &ContentStore, wire: &mut Wire) {
        self.watch_store(store);
        let held = &self.held;
        let outgoing = store.outgoing(self.instance, |block, frame| {
            held.replica(block)
                .is_none_or(|op| wire.relay(op, frame))
        });
        for (block, head) in outgoing.heads {
            self.held.offer(
                Subject::Head(block),
                &BlockValue::Head(head.map_err(be_sdk_api::Failure::Failed)),
            );
        }
        for (query, listed) in outgoing.listings {
            self.held.offer(
                Subject::Blocks(query.encode()),
                &BlockValue::Blocks(Ok(listed)),
            );
        }
        let waiting = wire.waiting();
        for (op, value) in blocks::answers(&mut self.linked(store, None), &waiting) {
            wire.answer_watch(op, value);
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
