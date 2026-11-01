use crate::{
    ArtifactState, BlockCommand, BlockInfo, BlockLocation, BlockQuery, ChildContent,
    ChildPlacements, EditorInstanceId, FrameCommit, Opening, VersionCommand, blocks::OpenRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockIdRole {
    Existing,
    Created,
}

type Visit<'a> = dyn FnMut(EditorInstanceId, BlockIdRole, &mut [u8; 16]) + 'a;

impl FrameCommit {
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
        let instance = self.instance();
        let mut existing = |id: &mut [u8; 16]| visit(instance, BlockIdRole::Existing, id);
        match self {
            Self::Block(open) => {
                existing(&mut open.block_id);
                open.view_block.iter_mut().for_each(existing);
            }
            Self::Artifact(open) => {
                existing(&mut open.block_id);
                existing(&mut open.source);
            }
            Self::Picker(open) => {
                open.filter.excluded.iter_mut().for_each(&mut existing);
                if let Some(place) = &mut open.filter.place {
                    location(place, &mut existing);
                }
                location(&mut open.parent, &mut existing);
            }
            Self::Rename(open) => existing(&mut open.block_id),
            Self::Creation(_) => {}
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
            | Self::PreviewRevision { block, .. }
            | Self::RestoreRevision { block, .. }
            | Self::ExportBlock { block, .. }
            | Self::ReadPlainFile { block, .. }
            | Self::ListAccess { block, .. }
            | Self::SetAccess { block, .. }
            | Self::MakeContactLink { block, .. } => visit(block),
            Self::ReleaseRevision { preview, .. } => visit(preview),
            Self::ImportBlocks { parent, .. } => location(parent, visit),
            Self::CopyBlocks { source, parent, .. } => {
                visit(source);
                location(parent, visit);
            }
            Self::PickFile { .. }
            | Self::SaveFile { .. }
            | Self::Fetch { .. }
            | Self::ListData { .. }
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
            Self::Roots | Self::Detached | Self::Shared => {}
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
            Self::Place { parent, .. } => visit(BlockIdRole::Existing, parent),
            Self::Undo | Self::Redo | Self::Artifact { .. } | Self::Restore => {}
        }
    }
}

impl VersionCommand {
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
            Self::SetParent { block, parent, .. } => {
                visit(existing, block);
                parent.visit_block_ids(visit);
            }
            Self::Define { block, .. }
            | Self::WriteContent { block, .. }
            | Self::ReplicaNew { block, .. }
            | Self::HistoryWatchNew { block, .. }
            | Self::HeadWatchNew { block, .. }
            | Self::ArtifactWatchNew { block, .. } => visit(existing, block),
            Self::Act { block, command, .. } => {
                visit(existing, block);
                command.visit_block_ids(visit);
            }
            Self::Version { block, command, .. } => {
                visit(existing, block);
                command.visit_block_ids(visit);
            }
            Self::OpenBlock { request, .. } => request.visit_block_ids(visit),
            Self::BlocksWatchNew { query, .. } => query.visit_block_ids(visit),
            Self::ReplicaFrames { .. }
            | Self::ReplicaSend { .. }
            | Self::BlocksWatchNext { .. }
            | Self::HistoryWatchNext { .. }
            | Self::HeadWatchNext { .. }
            | Self::ArtifactWatchNext { .. }
            | Self::VersionWatchNew { .. }
            | Self::VersionWatchNext { .. }
            | Self::ShowRequestsNew { .. }
            | Self::ShowRequestsNext { .. } => {}
        }
    }

    pub fn changes(&self) -> bool {
        matches!(
            self,
            Self::CreateBlock { .. }
                | Self::SetParent { .. }
                | Self::Define { .. }
                | Self::WriteContent { .. }
                | Self::Act { .. }
                | Self::Version { .. }
                | Self::OpenBlock { .. }
        )
    }
}
