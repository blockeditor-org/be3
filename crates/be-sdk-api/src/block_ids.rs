use crate::{
    ArtifactState, BlockCommand, BlockInfo, BlockLocation, BlockQuery, ChildContent,
    ChildPlacements, EditorInstanceId, LayoutCommit, Opening, OpeningKind, SearchResults,
    VersionCommand, blocks::OpenRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockIdRole {
    Existing,
    Created,
}

type Visit<'a> = dyn FnMut(EditorInstanceId, BlockIdRole, &mut [u8; 16]) + 'a;

impl LayoutCommit {
    pub fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        for placements in &mut self.children {
            placements.visit_block_ids(visit);
        }
    }
}

impl ChildPlacements {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        for child in &mut self.children {
            match &mut child.content {
                ChildContent::Block {
                    block_id,
                    view_block,
                    ..
                } => {
                    visit(self.instance, BlockIdRole::Existing, block_id);
                    if let Some(view_block) = view_block {
                        visit(self.instance, BlockIdRole::Existing, view_block);
                    }
                }
                ChildContent::WebView(_)
                | ChildContent::Host(_)
                | ChildContent::Window(_)
                | ChildContent::Creation { .. }
                | ChildContent::DockTab(_) => {}
                ChildContent::ArtifactSettings { block_id } | ChildContent::Rename(block_id) => {
                    visit(self.instance, BlockIdRole::Existing, block_id);
                }
                ChildContent::Picker { filter, parent, .. } => {
                    let instance = self.instance;
                    let mut existing =
                        |id: &mut [u8; 16]| visit(instance, BlockIdRole::Existing, id);
                    filter.excluded.iter_mut().for_each(&mut existing);
                    if let Some(place) = &mut filter.place {
                        location(place, &mut existing);
                    }
                    location(parent, &mut existing);
                }
            }
        }
    }
}

impl Opening {
    pub fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        let instance = self.instance;
        let mut existing = |id: &mut [u8; 16]| visit(instance, BlockIdRole::Existing, id);
        match &mut self.kind {
            OpeningKind::Block(open) => {
                existing(&mut open.block_id);
                open.view_block.iter_mut().for_each(existing);
            }
            OpeningKind::Artifact(open) => {
                existing(&mut open.block_id);
                existing(&mut open.source);
            }
            OpeningKind::Picker(open) => {
                open.filter.excluded.iter_mut().for_each(&mut existing);
                if let Some(place) = &mut open.filter.place {
                    location(place, &mut existing);
                }
                location(&mut open.parent, &mut existing);
            }
            OpeningKind::Rename(open) => existing(&mut open.block_id),
            OpeningKind::Host(open) => existing(&mut open.block_id),
            OpeningKind::Creation(_) => {}
        }
    }
}

fn location(location: &mut BlockLocation, visit: &mut impl FnMut(&mut [u8; 16])) {
    if let BlockLocation::Block(block_id) = location {
        visit(block_id);
    }
}

impl crate::embed::host_services::Call {
    pub fn visit_block_ids(&mut self, visit: &mut impl FnMut(&mut [u8; 16])) {
        match self {
            Self::ListRevisions { block, .. }
            | Self::RevisionPreviewNew { block, .. }
            | Self::RestoreRevision { block, .. }
            | Self::ExportBlock { block, .. }
            | Self::ReadPlainFile { block, .. }
            | Self::ListAccess { block, .. }
            | Self::SetAccess { block, .. }
            | Self::TransferOwnership { block, .. }
            | Self::MakeContactLink { block, .. }
            | Self::ListShareLinks { block, .. }
            | Self::MakeShareLink { block, .. }
            | Self::RevokeShareLink { block, .. }
            | Self::CreateBlockAgent { block, .. }
            | Self::SetMute { block, .. } => visit(block),
            Self::ImportBlocks { parent, .. } => location(parent, visit),
            Self::CutBlocks { blocks, holder, .. } => {
                blocks.iter_mut().for_each(&mut *visit);
                visit(holder);
            }
            Self::PasteBlocks { blocks, parent, .. } => {
                blocks.iter_mut().for_each(&mut *visit);
                location(parent, visit);
            }
            Self::CopyBlocks { source, parent, .. } => {
                visit(source);
                location(parent, visit);
            }
            Self::PickFile { .. }
            | Self::RevisionPreviewOpened { .. }
            | Self::SaveFile { .. }
            | Self::RotateAgentToken { .. }
            | Self::RemoveAgent { .. }
            | Self::TerminalHostBinary { .. }
            | Self::TerminalHostBuild { .. }
            | Self::SigningBlock { .. }
            | Self::Sign { .. }
            | Self::Verify { .. }
            | Self::ListData { .. }
            | Self::SetPushDevice { .. }
            | Self::SetPhoneLayout { .. }
            | Self::ReadData { .. } => {}
        }
    }
}

type Ids<'a> = dyn FnMut(BlockIdRole, &mut [u8; 16]) + 'a;

impl BlockLocation {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        if let Self::Block(block_id) = self {
            visit(BlockIdRole::Existing, block_id);
        }
    }
}

impl BlockQuery {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        match self {
            Self::Children(block_id)
            | Self::References(block_id)
            | Self::Backrefs(block_id)
            | Self::Parents(block_id)
            | Self::Block(block_id)
            | Self::OpenSettings(block_id) => visit(BlockIdRole::Existing, block_id),
            Self::Roots | Self::Detached | Self::Shared | Self::Unfiled => {}
        }
    }
}

impl BlockInfo {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        visit(BlockIdRole::Existing, &mut self.block_id);
        if let Some(parent) = &mut self.parent {
            parent.visit_block_ids(visit);
        }
        for reference in &mut self.references {
            visit(BlockIdRole::Existing, reference);
        }
    }
}

impl SearchResults {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        for hit in &mut self.hits {
            visit(BlockIdRole::Existing, &mut hit.block_id);
        }
    }
}

impl ArtifactState {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        visit(BlockIdRole::Existing, &mut self.block_id);
        visit(BlockIdRole::Existing, &mut self.source);
    }
}

impl crate::AccessListing {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        for grant in &mut self.grants {
            if let Some(source) = &mut grant.source {
                visit(BlockIdRole::Existing, source);
            }
        }
    }
}

impl OpenRequest {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        visit(BlockIdRole::Existing, &mut self.block_id);
        if let Some(via) = &mut self.via {
            visit(BlockIdRole::Existing, via);
        }
    }
}

impl BlockCommand {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        match self {
            Self::Unlink { container } => visit(BlockIdRole::Existing, container),
            Self::Delete { source, .. } => source.visit_block_ids(visit),
            Self::Move {
                source,
                destination,
                ..
            } => {
                source.visit_block_ids(visit);
                visit(BlockIdRole::Existing, destination);
            }
            Self::Reorder { parent, after } => {
                visit(BlockIdRole::Existing, parent);
                if let Some(after) = after {
                    visit(BlockIdRole::Existing, after);
                }
            }
            Self::Place { parent, .. } => visit(BlockIdRole::Existing, parent),
            Self::Version(command) => command.visit_block_ids(visit),
            Self::SetParent(parent) => parent.visit_block_ids(visit),
            Self::Undo
            | Self::Redo
            | Self::History(_)
            | Self::Artifact { .. }
            | Self::Restore
            | Self::Share
            | Self::Leave
            | Self::CloseEditor
            | Self::Define { .. } => {}
        }
    }
}

impl BlockCommand {
    /// Whether a host component may act so on a block in its reach
    /// (`Call::may_be_made_by_a_host`): never to leave or share it, close an editor, regenerate
    /// artifacts, or reach anything the command finds or places itself.
    fn may_be_made_by_a_host(&self) -> bool {
        let block = |location: &BlockLocation| matches!(location, BlockLocation::Block(_));
        match self {
            Self::Undo
            | Self::Redo
            | Self::History(_)
            | Self::Unlink { .. }
            | Self::Reorder { .. }
            | Self::Place { .. }
            | Self::Restore
            | Self::Define { .. } => true,
            Self::Delete { source, .. } | Self::Move { source, .. } => block(source),
            Self::SetParent(parent) => block(parent),
            Self::Version(command) => command.may_be_made_by_a_host(),
            // An account-wide membership change, a person's dialog or editor, and artifacts
            // made where the service places them.
            Self::Leave | Self::Share | Self::CloseEditor | Self::Artifact { .. } => false,
        }
    }
}

impl VersionCommand {
    /// Within a checkout the host reaches; never a new block the service places (a fork, a
    /// checkout, a branch) nor the upstream repository.
    fn may_be_made_by_a_host(&self) -> bool {
        match self {
            Self::Adopt { .. }
            | Self::Resolve { .. }
            | Self::Finish { .. }
            | Self::Update
            | Self::Switch { .. }
            | Self::Merge { .. }
            | Self::Undo { .. } => true,
            Self::CreateBranch { .. }
            | Self::NewCheckout { .. }
            | Self::Fork
            | Self::PullUpstream
            | Self::PushUpstream => false,
        }
    }

    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        match self {
            Self::Adopt { block_id } | Self::Resolve { block_id, .. } => {
                visit(BlockIdRole::Existing, block_id);
            }
            Self::Finish { .. }
            | Self::Update
            | Self::Switch { .. }
            | Self::CreateBranch { .. }
            | Self::Merge { .. }
            | Self::Undo { .. }
            | Self::NewCheckout { .. }
            | Self::Fork
            | Self::PullUpstream
            | Self::PushUpstream => {}
        }
    }
}

impl crate::embed::blocks::Call {
    pub fn visit_block_ids(&mut self, visit: &mut Ids<'_>) {
        let existing = BlockIdRole::Existing;
        match self {
            Self::CreateBlock { block, parent, .. } => {
                parent.visit_block_ids(visit);
                visit(BlockIdRole::Created, block);
            }
            Self::WriteContent { block, .. } | Self::ReplicaNew { block, .. } => {
                visit(existing, block)
            }
            Self::Act { block, command, .. } => {
                visit(existing, block);
                command.visit_block_ids(visit);
            }
            Self::OpenBlock { request, .. } => request.visit_block_ids(visit),
            Self::BlockWatchNew { subject, .. } => match subject {
                crate::blocks::BlockSubject::Blocks(query) => query.visit_block_ids(visit),
                crate::blocks::BlockSubject::History(block)
                | crate::blocks::BlockSubject::Head(block)
                | crate::blocks::BlockSubject::Artifact(block) => visit(existing, block),
                crate::blocks::BlockSubject::Version | crate::blocks::BlockSubject::Members => {}
            },
            Self::SearchWatchNew { within, .. } => {
                if let Some(within) = within {
                    visit(existing, within);
                }
            }
            Self::ReplicaFrames { .. }
            | Self::ReplicaSend { .. }
            | Self::BlockWatchNext { .. }
            | Self::SearchWatchNext { .. }
            | Self::ShowRequestsNew { .. }
            | Self::ShowRequestsNext { .. } => {}
        }
    }

    /// Whether a host component, which reaches only its own block and the blocks below it and
    /// has no person before it, may make the call (guides/apps/host_services.md, "Host
    /// components"); the block ids it names are checked against that reach apart. Every variant
    /// is decided here, with no wildcard, so a new one is refused until someone decides.
    pub fn may_be_made_by_a_host(&self) -> bool {
        let block = |location: &BlockLocation| matches!(location, BlockLocation::Block(_));
        match self {
            Self::CreateBlock { parent, .. } => block(parent),
            Self::Act { command, .. } => command.may_be_made_by_a_host(),
            Self::BlockWatchNew { subject, .. } => match subject {
                crate::blocks::BlockSubject::Blocks(query) => match query {
                    BlockQuery::Children(_)
                    | BlockQuery::References(_)
                    | BlockQuery::Backrefs(_)
                    | BlockQuery::Parents(_)
                    | BlockQuery::Block(_)
                    | BlockQuery::OpenSettings(_) => true,
                    BlockQuery::Roots
                    | BlockQuery::Detached
                    | BlockQuery::Shared
                    | BlockQuery::Unfiled => false,
                },
                crate::blocks::BlockSubject::History(_)
                | crate::blocks::BlockSubject::Head(_)
                | crate::blocks::BlockSubject::Artifact(_)
                | crate::blocks::BlockSubject::Version => true,
                // Names are for a person to read, and a host has none before it.
                crate::blocks::BlockSubject::Members => false,
            },
            Self::SearchWatchNew { within, .. } => within.is_some(),
            Self::WriteContent { .. }
            | Self::ReplicaNew { .. }
            | Self::ReplicaFrames { .. }
            | Self::ReplicaSend { .. }
            | Self::BlockWatchNext { .. }
            | Self::SearchWatchNext { .. }
            | Self::ShowRequestsNew { .. }
            | Self::ShowRequestsNext { .. } => true,
            // Shows a block to a person.
            Self::OpenBlock { .. } => false,
        }
    }

    pub fn changes(&self) -> bool {
        matches!(
            self,
            Self::CreateBlock { .. }
                | Self::WriteContent { .. }
                | Self::Act { .. }
                | Self::OpenBlock { .. }
        )
    }
}
