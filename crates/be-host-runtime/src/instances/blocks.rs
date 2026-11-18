use be_sdk_api::{
    BlockCommand, BlockIdRole, VersionCommand,
    blocks::{BlockHistory, BlockValue, OpenRequest, ShowRequest},
    embed::blocks::Call,
    ops::{OpError, OpId},
};
use be_sdk_app_host::blocks::{self as dispatch, BlockStore, Created, EditorBlocks, Subject, Written};
use uuid::Uuid;

use super::{ContentLink, EditorInstanceId, Instance, Instances};
use crate::{EditorAction, events::InstanceEvent};

pub(super) type Translate<'a> = dyn FnMut(BlockIdRole, &mut [u8; 16]) + 'a;

impl Instance {
    fn artifact_blocks(&self) -> Vec<Uuid> {
        let mut blocks: Vec<Uuid> = self
            .held
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

    /// One `be3:blocks` call, through the library's dispatcher (`be_sdk_app_host::blocks`) over
    /// this runtime's `BlockStore`. Whether it changed what the host shows.
    pub fn blocks_call(&mut self, instance: EditorInstanceId, call: Call) -> bool {
        let dispatched = dispatch::call(self, instance, call);
        match dispatched.reply {
            Some(dispatch::Reply::Answer(op, outcome)) => self.services.answers.push((op, outcome)),
            Some(dispatch::Reply::Accepted(op)) => self.accepted.push(op),
            None => {}
        }
        dispatched.changed
    }

    /// One `blocks.act`. Permission checks are a match on the command: version control, a
    /// parent and metadata are checked and done here; the rest go to be-host, which checks
    /// what it does.
    fn act_on(
        &mut self,
        instance: EditorInstanceId,
        block: Uuid,
        command: BlockCommand,
    ) -> Result<bool, OpError> {
        match command {
            BlockCommand::Version(command) => {
                let own = self
                    .entries
                    .get(&instance)
                    .and_then(|entry| entry.role.block())
                    .is_some_and(|own| own.id == block);
                if !own || !self.editable(block) {
                    return Err(OpError::Denied);
                }
                if let VersionCommand::Adopt { block_id } = &command
                    && !self.editable(Uuid::from_bytes(*block_id))
                {
                    return Err(OpError::Denied);
                }
                be_host_stack::version(block, command);
                Ok(true)
            }
            BlockCommand::SetParent(parent) => {
                let Some(parent) = be_host_stack::app_graph::parent_of(parent) else {
                    return Err(OpError::Denied);
                };
                if !self.editable(block)
                    || parent.block().is_some_and(|parent| !self.editable(parent))
                {
                    return Err(OpError::Denied);
                }
                be_host_stack::set_parent(block, parent);
                Ok(true)
            }
            BlockCommand::Define { key, value } => {
                let key = Uuid::from_bytes(key);
                if !self.editable(block)
                    || key == be_block_content::LOCAL_ID.id
                    || key == be_block_content::CREATED_BY.id
                {
                    return Err(OpError::Denied);
                }
                be_host_stack::define(block, vec![(key, value)]);
                Ok(true)
            }
            command => {
                if !self.entries.contains_key(&instance) {
                    return Err(OpError::Denied);
                }
                crate::events::emit(
                    &self.app_id,
                    instance,
                    InstanceEvent::Action(EditorAction::Command { id: block, command }),
                );
                Ok(true)
            }
        }
    }

    pub fn ended(&mut self, op: OpId) {
        if self.fetch_ended(op) || self.preview_ended(op) {
            return;
        }
        dispatch::ended(self, op);
        self.owners.remove(&op);
    }

    pub fn answers(&mut self, waiting: &[OpId]) -> Vec<(OpId, Vec<u8>)> {
        dispatch::answers(self, waiting)
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
            entry.held.offer(Subject::Artifact(block), &BlockValue::Artifact(state));
        }
    }
}

/// be-host's blocks, to the library's dispatcher: its scope, permission and event policy, over
/// be-host-stack.
impl BlockStore for Instances {
    fn held(&mut self, instance: EditorInstanceId) -> Option<&mut EditorBlocks> {
        self.entries.get_mut(&instance).map(|entry| &mut entry.held)
    }

    fn holder(&self, op: OpId) -> Option<EditorInstanceId> {
        self.owners.get(&op).copied()
    }

    fn admit(&mut self, instance: EditorInstanceId, call: &mut Call) -> Result<(), OpError> {
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
            return Err(OpError::Denied);
        }
        if let Some(own) = self.hosting(instance) {
            // A host component runs unattended: it reaches only its own block and the blocks
            // below it, and shows nothing (guides/apps/host_services.md, "Host components").
            let mut outside = !call.may_be_made_by_a_host();
            call.visit_block_ids(&mut |role, id| {
                let block = Uuid::from_bytes(*id);
                outside |= role == BlockIdRole::Existing
                    && block != own
                    && !be_host_stack::with_graph(|graph| graph.is_within(block, own))
                        .unwrap_or(false);
            });
            if outside {
                return Err(OpError::Denied);
            }
        }
        Ok(())
    }

    fn open_block(
        &mut self,
        instance: EditorInstanceId,
        request: OpenRequest,
    ) -> Result<bool, OpError> {
        if !self.entries.contains_key(&instance) {
            return Err(OpError::Denied);
        }
        let open = super::OpenRequest {
            id: Uuid::from_bytes(request.block_id),
            block_type: Uuid::from_bytes(request.block_type),
            via: request.via.map(Uuid::from_bytes),
            editor: request.editor.map(Uuid::from_bytes),
            at: request.at,
        };
        crate::events::emit(
            &self.app_id,
            instance,
            InstanceEvent::Action(EditorAction::OpenBlock(open)),
        );
        Ok(true)
    }

    fn act(
        &mut self,
        instance: EditorInstanceId,
        block: Uuid,
        command: BlockCommand,
    ) -> Result<bool, OpError> {
        self.act_on(instance, block, command)
    }

    fn create_block(
        &mut self,
        instance: EditorInstanceId,
        created: Created,
    ) -> Result<bool, OpError> {
        let Created {
            block,
            block_type,
            parent,
            defined,
            content,
            owner_only,
        } = created;
        let Some(parent) = be_host_stack::app_graph::parent_of(parent) else {
            self.refuse_block_gift(instance, block);
            return Err(OpError::Denied);
        };
        if parent.block().is_some_and(|parent| !self.editable(parent))
            || (owner_only && !own_settings(parent))
            || be_host_stack::node(block).is_some()
        {
            self.refuse_block_gift(instance, block);
            return Err(OpError::Denied);
        }
        let mut defined = be_host_stack::app_graph::entries_of(defined);
        defined.remove(&be_block_content::LOCAL_ID.id);
        let mut metadata = be_block_content::BlockMetadata {
            defined,
            ..be_block_content::BlockMetadata::default()
        };
        metadata.put(&be_block_content::CREATED_BY, self.created_by().as_ref());
        be_host_stack::create(block, block_type, parent, metadata, content, owner_only);
        Ok(true)
    }

    fn write_content(
        &mut self,
        instance: EditorInstanceId,
        written: Written,
    ) -> Result<bool, OpError> {
        let Written {
            block,
            content_type,
            bytes,
            replace,
        } = written;
        let allowed = match replace {
            true => self.editable(block),
            false => self
                .entries
                .get(&instance)
                .is_some_and(|entry| entry.holds(block)),
        };
        if !allowed || !be_host_stack::is_known(content_type) {
            return Err(OpError::Denied);
        }
        match replace {
            true => be_host_stack::replace(block, content_type, bytes),
            false => be_host_stack::seed(block, content_type, bytes),
        }
        Ok(false)
    }

    fn replica_opened(
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

    fn replica_sent(&mut self, instance: EditorInstanceId, block: Uuid, frame: Vec<u8>) -> bool {
        self.replica_frame(instance, block, frame)
    }

    fn replica_resumed(
        &mut self,
        instance: EditorInstanceId,
        op: OpId,
        block: Uuid,
        content_type: Uuid,
    ) {
        // What the stack queued went to the replica that ended.
        let kept = be_host_stack::take_editor_frames(instance.0)
            .into_iter()
            .filter(|(of, _)| *of != block)
            .collect();
        be_host_stack::keep_editor_frames(instance.0, kept);
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        let own = entry.role.block().is_some_and(|own| own.id == block);
        let same = entry
            .link_mut(block)
            .is_some_and(|link| own || link.content_type == content_type);
        if !same {
            if !(be_host_stack::provided(content_type) && be_host_stack::may_load(block)) {
                // Its content type is not one the host links: the later replica's link goes, as
                // it would have had the earlier one been alone, so nothing of another type
                // reaches it.
                self.replica_ended(instance, block);
                return;
            }
            // Linked for the later replica's content type: linked afresh for this one's, as a
            // replica opened anew is.
            self.replica_opened(instance, op, block, content_type);
        }
        if let Some(link) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.link_mut(block))
        {
            link.resuming = true;
        }
        // The replica answers a resend with a snapshot, as it does an editor that lacks its
        // content, and a link not attached yet is attached, which sends one.
        self.replica_frame(
            instance,
            block,
            be_sdk_type_api::encode_frame(&super::FromEditor::Resend),
        );
    }

    fn replica_ended(&mut self, instance: EditorInstanceId, block: Uuid) {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        if let Some(mut link) = entry.watched.remove(&block) {
            let shown = entry.shown_on(block);
            link.detach(instance, block, &shown);
            entry.shown.retain(|(on, _)| *on != block);
        }
    }

    fn watch_opened(&mut self, instance: EditorInstanceId, op: OpId, subject: Subject) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        self.owners.insert(op, instance);
        match subject {
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

    fn search_opened(
        &mut self,
        instance: EditorInstanceId,
        op: OpId,
        text: String,
        within: Option<Uuid>,
    ) -> bool {
        if let Some(entry) = self.entries.get_mut(&instance) {
            entry
                .searches
                .insert(op, be_host_stack::SearchAsk { text, within });
        }
        self.watch_opened(instance, op, Subject::Search(op))
    }

    fn watch_ended(&mut self, instance: EditorInstanceId, subject: Subject) {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        match subject {
            Subject::Head(block) => {
                if !entry
                    .held
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
    }

    fn take_shows(&mut self, instance: EditorInstanceId) -> Vec<ShowRequest> {
        if self
            .entries
            .get(&instance)
            .is_none_or(|entry| entry.shows.is_empty())
        {
            return Vec::new();
        }
        let scope = self.scope(instance);
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
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
        shows
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
