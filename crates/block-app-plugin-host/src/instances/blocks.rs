use block_plugin_api::{
    BlockIdRole, BlockQuery, VersionCommand,
    blocks::{BlockHistory, OpenRequest, ShowRequest},
    embed::blocks::Call,
    ops::{OpId, Serialize, encode},
};
use uuid::Uuid;

use super::{ContentLink, EditorInstanceId, Instance, Instances};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Subject {
    Blocks(BlockQuery),
    History(Uuid),
    Head(Uuid),
    Artifact(Uuid),
    Version,
    Shows,
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

    pub(super) fn offer(&mut self, watched: Subject, value: &impl Serialize) {
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
        let history = block_app_stack::history(block);
        BlockHistory {
            head: block_app_stack::node(block)
                .and_then(|node| node.head)
                .map(|head| *head.hash().as_bytes()),
            can_undo: history.can_undo,
            can_redo: history.can_redo,
        }
    }
}

impl Instances {
    pub(super) fn scope(&self, instance: EditorInstanceId) -> Option<block_app_stack::Scope> {
        let block = self.entries.get(&instance)?.role.block()?.id;
        let source = block_app_stack::revisions::source(block).unwrap_or(block);
        block_app_stack::with_graph(|graph| graph.scope_of(source)).flatten()
    }

    pub(super) fn translated(
        scope: Option<block_app_stack::Scope>,
        inward: bool,
        visit: impl FnOnce(&mut Translate<'_>),
    ) {
        let Some(scope) = scope else {
            return;
        };
        block_app_stack::with_graph(|graph| {
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
        if let Call::ReplicaSend { reply, .. } = &call {
            self.accepted.push(reply.op());
        }
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
            .is_some_and(|block| block_app_stack::revisions::source(block.id).is_some());
        if preview && call.changes() {
            return false;
        }
        match call {
            Call::OpenBlock { request } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.opens.push(super::OpenRequest {
                    id: Uuid::from_bytes(request.block_id),
                    block_type: Uuid::from_bytes(request.block_type),
                    via: request.via.map(Uuid::from_bytes),
                    editor: request.editor.map(Uuid::from_bytes),
                });
                true
            }
            Call::Act { block, command } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry
                    .block_commands
                    .push((Uuid::from_bytes(block), command));
                true
            }
            Call::Version { block, command } => {
                let block = Uuid::from_bytes(block);
                let own = self
                    .entries
                    .get(&instance)
                    .and_then(|entry| entry.role.block())
                    .is_some_and(|own| own.id == block);
                if !own || !self.editable(block) {
                    return false;
                }
                if let VersionCommand::Adopt { block_id } = &command
                    && !self.editable(Uuid::from_bytes(*block_id))
                {
                    return false;
                }
                block_app_stack::version(block, command);
                true
            }
            Call::WriteContent {
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
                if allowed && block_app_stack::is_known(content_type) {
                    match replace {
                        true => block_app_stack::replace(block, content_type, bytes),
                        false => block_app_stack::seed(block, content_type, bytes),
                    }
                }
                false
            }
            Call::CreateBlock {
                block,
                block_type,
                parent,
                defined,
                content,
            } => {
                let parent = block_app_stack::plugin_graph::parent_of(parent);
                if parent.block().is_some_and(|parent| !self.editable(parent)) {
                    return false;
                }
                let block = Uuid::from_bytes(block);
                if block_app_stack::node(block).is_some() {
                    return false;
                }
                let mut defined = block_app_stack::plugin_graph::entries_of(defined);
                defined.remove(&be_block::LOCAL_ID.id);
                let metadata = be_block::BlockMetadata {
                    defined,
                    ..be_block::BlockMetadata::default()
                };
                block_app_stack::create(
                    block,
                    Uuid::from_bytes(block_type),
                    parent,
                    metadata,
                    content,
                );
                true
            }
            Call::SetParent { block, parent } => {
                let block = Uuid::from_bytes(block);
                let parent = block_app_stack::plugin_graph::parent_of(parent);
                if !self.editable(block)
                    || parent.block().is_some_and(|parent| !self.editable(parent))
                {
                    return false;
                }
                block_app_stack::set_parent(block, parent);
                true
            }
            Call::Define { block, key, value } => {
                let block = Uuid::from_bytes(block);
                let key = Uuid::from_bytes(key);
                if !self.editable(block) || key == be_block::LOCAL_ID.id {
                    return false;
                }
                block_app_stack::define(block, vec![(key, value)]);
                true
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
            Call::ReplicaSend { this, frame, .. } => {
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
            Call::BlocksWatchNew { resource, query } => {
                self.watch(instance, resource.op(), Subject::Blocks(query))
            }
            Call::HistoryWatchNew { resource, block } => self.watch(
                instance,
                resource.op(),
                Subject::History(Uuid::from_bytes(block)),
            ),
            Call::HeadWatchNew { resource, block } => self.watch(
                instance,
                resource.op(),
                Subject::Head(Uuid::from_bytes(block)),
            ),
            Call::ArtifactWatchNew { resource, block } => self.watch(
                instance,
                resource.op(),
                Subject::Artifact(Uuid::from_bytes(block)),
            ),
            Call::VersionWatchNew { resource } => {
                self.watch(instance, resource.op(), Subject::Version)
            }
            Call::ShowRequestsNew { resource } => {
                self.watch(instance, resource.op(), Subject::Shows)
            }
            Call::ReplicaFrames { .. }
            | Call::BlocksWatchNext { .. }
            | Call::HistoryWatchNext { .. }
            | Call::HeadWatchNext { .. }
            | Call::ArtifactWatchNext { .. }
            | Call::VersionWatchNext { .. }
            | Call::ShowRequestsNext { .. } => false,
        }
    }

    fn watch(&mut self, instance: EditorInstanceId, op: OpId, watched: Subject) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        self.owners.insert(op, instance);
        entry.watches.insert(op, Watch::new(watched));
        match watched {
            Subject::Blocks(_) => entry.blocks_seen = None,
            Subject::Head(block) => entry.heads.watch(block),
            Subject::Artifact(_) => entry.artifact_watch = Some(entry.artifact_blocks()),
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
        let allowed = block_app_stack::provided(content_type) && block_app_stack::may_load(block);
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
                Subject::Artifact(_) => entry.artifact_watch = Some(entry.artifact_blocks()),
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
                    if let ShowRequest::Block(request) = show {
                        request.visit_block_ids(visit);
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

    pub fn show_panel(&mut self, instance: EditorInstanceId, panel: block_plugin_api::HostPanel) {
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
        states: Vec<block_plugin_api::ArtifactState>,
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
            entry.offer(Subject::Artifact(block), &state);
        }
    }
}
