use be_sdk_api::{
    BlockCommand, BlockIdRole, BlockQuery, VersionCommand,
    blocks::{BlockHistory, BlockSubject, BlockValue, OpenRequest, ShowRequest},
    embed::blocks::Call,
    ops::{OpError, OpId, Outcome, Reply, Serialize, encode},
};
use uuid::Uuid;

use super::{ContentLink, EditorInstanceId, Instance, Instances};
use crate::{EditorAction, events::InstanceEvent};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Subject {
    Blocks(BlockQuery),
    History(Uuid),
    Head(Uuid),
    Artifact(Uuid),
    Version,
    Shows,
    /// A full-text search, named by its watch: `Instance::searches` holds what it asks.
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
    fn answered_by(self, value: &BlockValue) -> bool {
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

pub(super) struct Watch {
    pub(super) watched: Subject,
    answered: Option<Vec<u8>>,
    latest: Option<Vec<u8>>,
}

impl Watch {
    fn new(watched: Subject) -> Self {
        Self {
            watched,
            answered: None,
            latest: None,
        }
    }

    pub(super) fn offer(&mut self, value: &impl Serialize) {
        let bytes = encode(value);
        self.latest = (self.answered.as_ref() != Some(&bytes)).then_some(bytes);
    }

    fn take(&mut self) -> Option<Vec<u8>> {
        let bytes = self.latest.take()?;
        self.answered = Some(bytes.clone());
        Some(bytes)
    }
}

pub(super) type Translate<'a> = dyn FnMut(BlockIdRole, &mut [u8; 16]) + 'a;

impl Instance {
    pub(super) fn watching(&self) -> impl Iterator<Item = Subject> + '_ {
        self.watches.values().map(|watch| watch.watched)
    }

    /// Offers the block watches of `watched` its latest value, a `BlockValue` of the
    /// variant of the same name.
    pub(super) fn offer(&mut self, watched: Subject, value: &BlockValue) {
        debug_assert!(watched.answered_by(value), "{watched:?} offered {value:?}");
        self.offer_encoded(watched, value);
    }

    /// Offers the search watch `op` its latest results.
    pub(super) fn offer_search(&mut self, op: OpId, results: &be_sdk_api::SearchResults) {
        self.offer_encoded(Subject::Search(op), results);
    }

    fn offer_encoded(&mut self, watched: Subject, value: &impl Serialize) {
        for watch in self.watches.values_mut() {
            if watch.watched == watched {
                watch.offer(value);
            }
        }
    }

    fn artifact_blocks(&self) -> Vec<Uuid> {
        let mut blocks: Vec<Uuid> = self
            .watching()
            .filter_map(|watched| match watched {
                Subject::Artifact(block) => Some(block),
                _ => None,
            })
            .collect();
        blocks.sort();
        blocks.dedup();
        blocks
    }

    pub(super) fn history(&self, block: Uuid) -> BlockHistory {
        let history = be_host_stack::history(block);
        BlockHistory {
            head: be_host_stack::node(block)
                .and_then(|node| node.head)
                .map(|head| *head.hash().as_bytes()),
            can_undo: history.can_undo,
            can_redo: history.can_redo,
            restarted_at: history.restarted_at.map(|commit| *commit.hash().as_bytes()),
            lost_edits: history.lost_edits,
        }
    }
}

impl Instances {
    /// What the host records as having made a block this runtime's app creates, imports or
    /// copies (`CREATED_BY`): its manifest id, never what the app says.
    pub(super) fn created_by(&self) -> Option<String> {
        (!self.app_id.is_empty()).then(|| self.app_id.clone())
    }

    pub(super) fn scope(&self, instance: EditorInstanceId) -> Option<be_host_stack::Scope> {
        let block = self.entries.get(&instance)?.role.block()?.id;
        let source = be_host_stack::revisions::source(block).unwrap_or(block);
        be_host_stack::with_graph(|graph| graph.scope_of(source)).flatten()
    }

    pub(super) fn translated(
        scope: Option<be_host_stack::Scope>,
        inward: bool,
        visit: impl FnOnce(&mut Translate<'_>),
    ) {
        let Some(scope) = scope else {
            return;
        };
        be_host_stack::with_graph(|graph| {
            visit(&mut |role, id| {
                let block = Uuid::from_bytes(*id);
                let translated = match inward {
                    true => graph.to_real(scope, block, role),
                    false => graph.to_local(scope, block),
                };
                *id = translated.into_bytes();
            });
        });
    }

    pub fn blocks_call(&mut self, instance: EditorInstanceId, mut call: Call) -> bool {
        let mut carries = false;
        call.visit_block_ids(&mut |_, _| carries = true);
        if carries {
            let scope = self.scope(instance);
            Self::translated(scope, true, |visit| call.visit_block_ids(visit));
        }
        let preview = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.role.block())
            .is_some_and(|block| be_host_stack::revisions::source(block.id).is_some());
        if preview && call.changes() {
            self.services.answers.push(call.refuse(OpError::Denied));
            return false;
        }
        match call {
            Call::OpenBlock { reply, request } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return self.acknowledge(reply, None);
                };
                let open = super::OpenRequest {
                    id: Uuid::from_bytes(request.block_id),
                    block_type: Uuid::from_bytes(request.block_type),
                    via: request.via.map(Uuid::from_bytes),
                    editor: request.editor.map(Uuid::from_bytes),
                    at: request.at,
                };
                let _ = entry;
                crate::events::emit(
                    &self.app_id,
                    instance,
                    InstanceEvent::Action(EditorAction::OpenBlock(open)),
                );
                self.acknowledge(reply, Some(true))
            }
            Call::Act {
                reply,
                block,
                command,
            } => self.act(instance, reply, Uuid::from_bytes(block), command),
            Call::WriteContent {
                reply,
                block,
                content_type,
                bytes,
                replace,
            } => {
                let block = Uuid::from_bytes(block);
                let content_type = Uuid::from_bytes(content_type);
                let allowed = match replace {
                    true => self.editable(block),
                    false => self
                        .entries
                        .get(&instance)
                        .is_some_and(|entry| entry.holds(block)),
                };
                if !allowed || !be_host_stack::is_known(content_type) {
                    return self.acknowledge(reply, None);
                }
                match replace {
                    true => be_host_stack::replace(block, content_type, bytes),
                    false => be_host_stack::seed(block, content_type, bytes),
                }
                self.acknowledge(reply, Some(false))
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
                let block = Uuid::from_bytes(block);
                let Some(parent) = be_host_stack::app_graph::parent_of(parent) else {
                    self.refuse_block_gift(instance, block);
                    return self.acknowledge(reply, None);
                };
                if parent.block().is_some_and(|parent| !self.editable(parent))
                    || (owner_only && !own_settings(parent))
                    || be_host_stack::node(block).is_some()
                {
                    self.refuse_block_gift(instance, block);
                    return self.acknowledge(reply, None);
                }
                let mut defined = be_host_stack::app_graph::entries_of(defined);
                defined.remove(&be_block::LOCAL_ID.id);
                let mut metadata = be_block::BlockMetadata {
                    defined,
                    ..be_block::BlockMetadata::default()
                };
                metadata.put(&be_block::CREATED_BY, self.created_by().as_ref());
                be_host_stack::create(
                    block,
                    Uuid::from_bytes(block_type),
                    parent,
                    metadata,
                    content,
                    owner_only,
                );
                self.acknowledge(reply, Some(true))
            }
            Call::ReplicaNew {
                resource,
                block,
                content_type,
            } => self.hold_replica(
                instance,
                resource.op(),
                Uuid::from_bytes(block),
                Uuid::from_bytes(content_type),
            ),
            Call::ReplicaSend { reply, this, frame } => {
                self.accepted.push(reply.into_op());
                let block = self.entries.get(&instance).and_then(|entry| {
                    entry
                        .replicas
                        .iter()
                        .find(|(_, op)| **op == this.op())
                        .map(|(block, _)| *block)
                });
                match block {
                    Some(block) => self.replica_frame(instance, block, frame),
                    None => false,
                }
            }
            Call::BlockWatchNew { resource, subject } => {
                self.watch(instance, resource.op(), subject.into())
            }
            Call::SearchWatchNew {
                resource,
                text,
                within,
            } => {
                let op = resource.op();
                if let Some(entry) = self.entries.get_mut(&instance) {
                    entry.searches.insert(
                        op,
                        be_host_stack::SearchAsk {
                            text,
                            within: within.map(Uuid::from_bytes),
                        },
                    );
                }
                self.watch(instance, op, Subject::Search(op))
            }
            Call::ShowRequestsNew { resource } => {
                self.watch(instance, resource.op(), Subject::Shows)
            }
            call @ (Call::ReplicaFrames { .. }
            | Call::BlockWatchNext { .. }
            | Call::SearchWatchNext { .. }
            | Call::ShowRequestsNext { .. }) => {
                self.services
                    .answers
                    .push(call.refuse(OpError::Unsupported));
                false
            }
        }
    }

    /// One `blocks.act`. Permission checks are a match on the command: version control, a
    /// parent and metadata are checked and done here; the rest go to be-host, which checks
    /// what it does.
    fn act(
        &mut self,
        instance: EditorInstanceId,
        reply: Reply<()>,
        block: Uuid,
        command: BlockCommand,
    ) -> bool {
        match command {
            BlockCommand::Version(command) => {
                let own = self
                    .entries
                    .get(&instance)
                    .and_then(|entry| entry.role.block())
                    .is_some_and(|own| own.id == block);
                if !own || !self.editable(block) {
                    return self.acknowledge(reply, None);
                }
                if let VersionCommand::Adopt { block_id } = &command
                    && !self.editable(Uuid::from_bytes(*block_id))
                {
                    return self.acknowledge(reply, None);
                }
                be_host_stack::version(block, command);
                self.acknowledge(reply, Some(true))
            }
            BlockCommand::SetParent(parent) => {
                let Some(parent) = be_host_stack::app_graph::parent_of(parent) else {
                    return self.acknowledge(reply, None);
                };
                if !self.editable(block)
                    || parent.block().is_some_and(|parent| !self.editable(parent))
                {
                    return self.acknowledge(reply, None);
                }
                be_host_stack::set_parent(block, parent);
                self.acknowledge(reply, Some(true))
            }
            BlockCommand::Define { key, value } => {
                let key = Uuid::from_bytes(key);
                if !self.editable(block)
                    || key == be_block::LOCAL_ID.id
                    || key == be_block::CREATED_BY.id
                {
                    return self.acknowledge(reply, None);
                }
                be_host_stack::define(block, vec![(key, value)]);
                self.acknowledge(reply, Some(true))
            }
            command => {
                if !self.entries.contains_key(&instance) {
                    return self.acknowledge(reply, None);
                }
                crate::events::emit(
                    &self.app_id,
                    instance,
                    InstanceEvent::Action(EditorAction::Command { id: block, command }),
                );
                self.acknowledge(reply, Some(true))
            }
        }
    }

    fn acknowledge(&mut self, reply: Reply<()>, changed: Option<bool>) -> bool {
        self.services.answers.push(match changed {
            Some(_) => reply.answer(&()),
            None => (reply.into_op(), Outcome::Err(OpError::Denied)),
        });
        changed.unwrap_or(false)
    }

    fn watch(&mut self, instance: EditorInstanceId, op: OpId, watched: Subject) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        self.owners.insert(op, instance);
        entry.watches.insert(op, Watch::new(watched));
        match watched {
            Subject::Blocks(_) => entry.blocks_due = true,
            Subject::Search(_) => entry.searches_due = true,
            Subject::Head(block) => entry.heads.watch(block),
            Subject::Artifact(_) => crate::events::emit(
                &self.app_id,
                instance,
                InstanceEvent::ArtifactWatch(entry.artifact_blocks()),
            ),
            Subject::Version => entry.version_sent = None,
            Subject::History(_) | Subject::Shows => {}
        }
        entry.stale = true;
        true
    }

    fn hold_replica(
        &mut self,
        instance: EditorInstanceId,
        op: OpId,
        block: Uuid,
        content_type: Uuid,
    ) -> bool {
        let allowed = be_host_stack::provided(content_type) && be_host_stack::may_load(block);
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        self.owners.insert(op, instance);
        entry.replicas.insert(block, op);
        if entry.role.block().is_some_and(|own| own.id == block) || !allowed {
            return true;
        }
        let kept = entry
            .watched
            .get(&block)
            .is_some_and(|link| link.content_type == content_type);
        if !kept {
            if let Some(mut link) = entry.watched.remove(&block) {
                let shown = entry.shown_on(block);
                link.detach(instance, block, &shown);
                entry.shown.retain(|(on, _)| *on != block);
            }
            entry.watched.insert(block, ContentLink::new(content_type));
        }
        entry.stale = true;
        true
    }

    pub fn ended(&mut self, op: OpId) {
        if self.fetch_ended(op) {
            return;
        }
        let Some(instance) = self.owners.remove(&op) else {
            return;
        };
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        if let Some(watch) = entry.watches.remove(&op) {
            match watch.watched {
                Subject::Head(block) => {
                    if !entry
                        .watching()
                        .any(|watched| watched == Subject::Head(block))
                    {
                        entry.heads.unwatch(block);
                    }
                }
                Subject::Artifact(_) => crate::events::emit(
                    &self.app_id,
                    instance,
                    InstanceEvent::ArtifactWatch(entry.artifact_blocks()),
                ),
                Subject::Search(op) => {
                    entry.searches.remove(&op);
                }
                Subject::Blocks(_) | Subject::History(_) | Subject::Version | Subject::Shows => {}
            }
            return;
        }
        let Some(block) = entry
            .replicas
            .iter()
            .find(|(_, held)| **held == op)
            .map(|(block, _)| *block)
        else {
            return;
        };
        entry.replicas.remove(&block);
        if let Some(mut link) = entry.watched.remove(&block) {
            let shown = entry.shown_on(block);
            link.detach(instance, block, &shown);
            entry.shown.retain(|(on, _)| *on != block);
        }
    }

    pub fn answers(&mut self, waiting: &[OpId]) -> Vec<(OpId, Vec<u8>)> {
        let mut answers = Vec::new();
        for op in waiting {
            let Some(instance) = self.owners.get(op).copied() else {
                continue;
            };
            let scope = match self.entries.get(&instance) {
                Some(entry) if !entry.shows.is_empty() => self.scope(instance),
                _ => None,
            };
            let Some(entry) = self.entries.get_mut(&instance) else {
                continue;
            };
            let Some(watch) = entry.watches.get_mut(op) else {
                continue;
            };
            if watch.watched != Subject::Shows {
                answers.extend(watch.take().map(|bytes| (*op, bytes)));
                continue;
            }
            if entry.shows.is_empty() {
                continue;
            }
            let mut shows = std::mem::take(&mut entry.shows);
            Self::translated(scope, false, |visit| {
                for show in &mut shows {
                    match show {
                        ShowRequest::Block(request) => request.visit_block_ids(visit),
                        ShowRequest::History(request) => {
                            visit(BlockIdRole::Existing, &mut request.block_id)
                        }
                        ShowRequest::Panel(_) => {}
                    }
                }
            });
            answers.push((*op, encode(&shows)));
        }
        answers
    }

    pub fn relay(&mut self, mut send: impl FnMut(OpId, &Vec<u8>) -> bool) {
        for instance in &self.relaying {
            if let Some(entry) = self.entries.get_mut(instance) {
                entry.relay_frames(*instance, &mut send);
            }
        }
    }

    pub fn take_accepted(&mut self) -> Vec<OpId> {
        std::mem::take(&mut self.accepted)
    }

    /// Asks a shell to show a block's version history, at `commit` when named.
    pub fn show_history(
        &mut self,
        instance: EditorInstanceId,
        block: Uuid,
        commit: Option<[u8; 32]>,
    ) {
        if let Some(entry) = self.entries.get_mut(&instance) {
            ShowRequest::queue(
                &mut entry.shows,
                ShowRequest::History(be_sdk_api::blocks::HistoryRequest {
                    block_id: block.into_bytes(),
                    commit,
                }),
            );
        }
    }

    pub fn show_panel(&mut self, instance: EditorInstanceId, panel: be_sdk_api::HostPanel) {
        if let Some(entry) = self.entries.get_mut(&instance) {
            ShowRequest::queue(&mut entry.shows, ShowRequest::Panel(panel));
        }
    }

    pub fn show_block(&mut self, instance: EditorInstanceId, open: super::OpenRequest) {
        let request = ShowRequest::Block(OpenRequest {
            block_id: open.id.into_bytes(),
            block_type: open.block_type.into_bytes(),
            via: open.via.map(Uuid::into_bytes),
            editor: open.editor.map(Uuid::into_bytes),
            at: open.at,
        });
        match self.entries.get_mut(&instance) {
            Some(entry) => ShowRequest::queue(&mut entry.shows, request),
            None if !self.gone.contains(&instance) => {
                ShowRequest::queue(self.early_shows.entry(instance).or_default(), request);
            }
            None => {}
        }
    }

    pub fn set_artifact_states(
        &mut self,
        instance: EditorInstanceId,
        states: Vec<be_sdk_api::ArtifactState>,
    ) {
        let scope = self.scope(instance);
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        for block in entry.artifact_blocks() {
            let mut state = states
                .iter()
                .find(|state| state.block_id == block.into_bytes())
                .cloned();
            if let Some(state) = &mut state {
                Self::translated(scope, false, |visit| state.visit_block_ids(visit));
            }
            entry.offer(Subject::Artifact(block), &BlockValue::Artifact(state));
        }
    }
}

/// Whether `parent` is a block of this account's own settings, the only place an owner-only
/// block is made (guides/settings.md, Owner only).
fn own_settings(parent: be_block_graph::BlockParent) -> bool {
    let account = be_host_stack::account();
    parent
        .block()
        .and_then(be_host_stack::node)
        .is_some_and(|node| {
            node.author.is_some()
                && node.author == account
                && be_block::holds_owner_only(node.content_type)
        })
}
