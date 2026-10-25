use crate::{
    BlockCommand, BlockLocation, BlockQuery, ChildContent, ChildPlacements, EditorInstanceId,
    EditorMessage, Message, VersionCommand,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockIdRole {
    Existing,
    Created,
}

type Visit<'a> = dyn FnMut(EditorInstanceId, BlockIdRole, &mut [u8; 16]) + 'a;

impl Message {
    pub fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        match self {
            Self::Editor(message) => message.visit_block_ids(visit),
            Self::Frame(commit) => {
                for placements in &mut commit.children {
                    placements.visit_block_ids(visit);
                }
            }
            Self::Hello(_)
            | Self::HelloAccepted(_)
            | Self::Preferences(_)
            | Self::UtcOffset(_)
            | Self::Fonts(_)
            | Self::MissingCharacters(_)
            | Self::Screens(_)
            | Self::Configure { .. }
            | Self::Layout(_)
            | Self::Input(_)
            | Self::DrawFrame { .. }
            | Self::FrameNeeded
            | Self::Acknowledged { .. }
            | Self::Error(_)
            | Self::Shutdown
            | Self::ShutdownAcknowledged
            | Self::Describe(_)
            | Self::RunAction { .. }
            | Self::NodeAction { .. }
            | Self::Extension { .. } => {}
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
                | ChildContent::Creation { .. } => {}
                ChildContent::ArtifactSettings { block_id } => {
                    visit(self.instance, BlockIdRole::Existing, block_id);
                }
            }
        }
    }
}

impl EditorMessage {
    pub fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        let instance = self.instance();
        let mut existing = |id: &mut [u8; 16]| visit(instance, BlockIdRole::Existing, id);
        match self {
            Self::Open {
                block_id,
                view_block,
                ..
            } => {
                existing(block_id);
                view_block.iter_mut().for_each(existing);
            }
            Self::SeedContent { block_id, .. }
            | Self::ReplaceContent { block_id, .. }
            | Self::OpenArtifact { block_id, .. }
            | Self::Define { block_id, .. }
            | Self::VersionStatus { block_id, .. }
            | Self::Head { block_id, .. }
            | Self::Replica { block_id, .. } => existing(block_id),
            Self::OpenBlock { block_id, via, .. } | Self::ShowBlock { block_id, via, .. } => {
                existing(block_id);
                via.iter_mut().for_each(existing);
            }
            Self::WatchContent { blocks, .. } => {
                for block in blocks {
                    existing(&mut block.block_id);
                }
            }
            Self::BlockCommand {
                block_id, command, ..
            } => {
                existing(block_id);
                match command {
                    BlockCommand::Unlink { container } => existing(container),
                    BlockCommand::Delete { source, .. } => location(source, &mut existing),
                    BlockCommand::Move {
                        source,
                        destination,
                        ..
                    } => {
                        location(source, &mut existing);
                        existing(destination);
                    }
                    BlockCommand::Place { parent, .. } => existing(parent),
                    BlockCommand::Share
                    | BlockCommand::Leave
                    | BlockCommand::Rename { .. }
                    | BlockCommand::Undo
                    | BlockCommand::Redo
                    | BlockCommand::Artifact { .. }
                    | BlockCommand::SimulateAccess { .. }
                    | BlockCommand::CloseEditor
                    | BlockCommand::Restore
                    | BlockCommand::AppMenu { .. } => {}
                }
            }
            Self::WatchArtifacts { blocks, .. }
            | Self::WatchHistory { blocks, .. }
            | Self::WatchHeads { blocks, .. } => {
                blocks.iter_mut().for_each(existing);
            }
            Self::ArtifactStates { states, .. } => {
                for state in states {
                    existing(&mut state.block_id);
                    state.source.iter_mut().for_each(&mut existing);
                }
            }
            Self::HistoryStates { states, .. } => {
                for state in states {
                    existing(&mut state.block_id);
                }
            }
            Self::WatchBlocks { queries, .. } => {
                for query in queries {
                    self::query(query, &mut existing);
                }
            }
            Self::BlocksFailed { query, .. } => self::query(query, &mut existing),
            Self::Blocks { query, blocks, .. } => {
                self::query(query, &mut existing);
                for block in blocks {
                    existing(&mut block.block_id);
                    if let Some(parent) = &mut block.parent {
                        location(parent, &mut existing);
                    }
                    block.references.iter_mut().for_each(&mut existing);
                }
            }
            Self::CreateBlock {
                block_id, parent, ..
            } => {
                location(parent, &mut existing);
                visit(instance, BlockIdRole::Created, block_id);
            }
            Self::SetParent {
                block_id, parent, ..
            } => {
                existing(block_id);
                location(parent, &mut existing);
            }
            Self::VersionControl {
                block_id, command, ..
            } => {
                existing(block_id);
                match command {
                    VersionCommand::Adopt { block_id }
                    | VersionCommand::Resolve { block_id, .. } => {
                        existing(block_id);
                    }
                    VersionCommand::Finish { .. }
                    | VersionCommand::Update
                    | VersionCommand::Switch { .. }
                    | VersionCommand::CreateBranch { .. }
                    | VersionCommand::Merge { .. }
                    | VersionCommand::Undo { .. }
                    | VersionCommand::NewCheckout { .. }
                    | VersionCommand::Fork
                    | VersionCommand::PullUpstream
                    | VersionCommand::PushUpstream => {}
                }
            }
            Self::Close { .. }
            | Self::AudioStatus { .. }
            | Self::PlayAudio { .. }
            | Self::WebViewCommand { .. }
            | Self::ShowPanel { .. }
            | Self::WatchHostValue { .. }
            | Self::HostValue { .. }
            | Self::HostAction { .. }
            | Self::WebViewEvent { .. }
            | Self::OpenCreation { .. } => {}
        }
    }
}

fn location(location: &mut BlockLocation, visit: &mut impl FnMut(&mut [u8; 16])) {
    if let BlockLocation::Block(block_id) = location {
        visit(block_id);
    }
}

fn query(query: &mut BlockQuery, visit: &mut impl FnMut(&mut [u8; 16])) {
    match query {
        BlockQuery::Children(block_id)
        | BlockQuery::References(block_id)
        | BlockQuery::Backrefs(block_id)
        | BlockQuery::Parents(block_id)
        | BlockQuery::Block(block_id) => visit(block_id),
        BlockQuery::Roots | BlockQuery::Detached | BlockQuery::Shared => {}
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
            Self::ReleaseRevision { preview } => visit(preview),
            Self::ImportBlocks { parent, .. } => location(parent, visit),
            Self::PickBlock { filter, .. } => {
                filter.excluded.iter_mut().for_each(&mut *visit);
                if let Some(place) = &mut filter.place {
                    location(place, visit);
                }
            }
            Self::PickFile { .. }
            | Self::SaveFile { .. }
            | Self::Fetch { .. }
            | Self::ListData { .. }
            | Self::ReadData { .. } => {}
        }
    }
}
