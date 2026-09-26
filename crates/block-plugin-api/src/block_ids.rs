use crate::{
    ArtifactDescription, BlockCommand, BlockLocation, BlockPick, BlockQuery, CreationOutcome,
    EditorInstanceId, EditorMessage, HostReply, HostRequest, Message, VersionCommand,
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
            Self::Children(placements) => {
                for child in &mut placements.children {
                    visit(
                        placements.instance,
                        BlockIdRole::Existing,
                        &mut child.block_id,
                    );
                }
            }
            Self::Hello(_)
            | Self::HelloAccepted(_)
            | Self::HelloRejected(_)
            | Self::Theme(_)
            | Self::Screens(_)
            | Self::Layout(_)
            | Self::RegionSizes(_)
            | Self::Frames(_)
            | Self::Input(_)
            | Self::DrawFrame
            | Self::FrameNeeded
            | Self::FrameReady(_)
            | Self::Acknowledged { .. }
            | Self::Error(_)
            | Self::Shutdown
            | Self::ShutdownAcknowledged
            | Self::BlockTypes(_)
            | Self::ChildStatuses(_) => {}
        }
    }
}

impl EditorMessage {
    pub fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        let instance = self.instance();
        let mut existing = |id: &mut [u8; 16]| visit(instance, BlockIdRole::Existing, id);
        match self {
            Self::Open { block_id, .. }
            | Self::Content { block_id, .. }
            | Self::ContentOperations { block_id, .. }
            | Self::Operate { block_id, .. }
            | Self::SeedContent { block_id, .. }
            | Self::ReplaceContent { block_id, .. }
            | Self::ShowPresence { block_id, .. }
            | Self::PeerPresence { block_id, .. }
            | Self::DragBlock { block_id, .. }
            | Self::DragOver { block_id, .. }
            | Self::PlayAudio { block_id, .. }
            | Self::OpenArtifact { block_id, .. }
            | Self::SetName { block_id, .. }
            | Self::VersionStatus { block_id, .. } => existing(block_id),
            Self::OpenBlock { block_id, via, .. } | Self::ShowBlock { block_id, via, .. } => {
                existing(block_id);
                via.iter_mut().for_each(existing);
            }
            Self::Focused { block_id, via, .. } | Self::FocusChanged { block_id, via, .. } => {
                block_id.iter_mut().for_each(&mut existing);
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
                    | BlockCommand::Rename
                    | BlockCommand::Undo
                    | BlockCommand::Redo
                    | BlockCommand::Artifact { .. }
                    | BlockCommand::SimulateAccess { .. }
                    | BlockCommand::CloseEditor => {}
                }
            }
            Self::Request { request, .. } => match request {
                HostRequest::PickBlock(filter) => filter.excluded.iter_mut().for_each(existing),
                HostRequest::PickFile(_) | HostRequest::PasteImage | HostRequest::Fetch(_) => {}
            },
            Self::Replied { reply, .. } => match reply {
                HostReply::BlockPicked(BlockPick::Chosen { block_id, .. }) => existing(block_id),
                HostReply::BlockPicked(BlockPick::Cancelled | BlockPick::Failed(_))
                | HostReply::FilePicked(_)
                | HostReply::ImagePasted(_)
                | HostReply::Fetched(_) => {}
            },
            Self::CreationBlock { outcome, .. } => match outcome {
                CreationOutcome::Created(block_id) => existing(block_id),
                CreationOutcome::Failed(_) => {}
            },
            Self::ArtifactDescribed { description, .. } => match description {
                ArtifactDescription::Described { source, .. } => existing(source),
                ArtifactDescription::Unreadable(_) => {}
            },
            Self::WatchArtifacts { blocks, .. } | Self::WatchHistory { blocks, .. } => {
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
            Self::ReplaceChild { old, new, .. } => {
                existing(old);
                existing(new);
            }
            Self::WatchBlocks { queries, .. } => {
                for query in queries {
                    self::query(query, &mut existing);
                }
            }
            Self::Blocks { query, blocks, .. } => {
                self::query(query, &mut existing);
                for block in blocks {
                    existing(&mut block.block_id);
                    location(&mut block.parent, &mut existing);
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
                    VersionCommand::Commit { .. }
                    | VersionCommand::Update
                    | VersionCommand::Switch { .. }
                    | VersionCommand::CreateBranch { .. }
                    | VersionCommand::NewCheckout { .. }
                    | VersionCommand::Fork
                    | VersionCommand::PullUpstream
                    | VersionCommand::PushUpstream => {}
                }
            }
            Self::EditabilityChanged { .. }
            | Self::ViewChanged { .. }
            | Self::ChangeView { .. }
            | Self::Present { .. }
            | Self::PresentingChanged { .. }
            | Self::Resized { .. }
            | Self::LeaveFrame { .. }
            | Self::Close { .. }
            | Self::DragLeft { .. }
            | Self::FileDrop { .. }
            | Self::FileDropLeft { .. }
            | Self::DragAccepted { .. }
            | Self::AudioStatus { .. }
            | Self::GrabCursor { .. }
            | Self::WebView { .. }
            | Self::WebViewCommand { .. }
            | Self::WebViewEvent { .. }
            | Self::OpenCreation { .. }
            | Self::CreationReady { .. }
            | Self::CommitCreation { .. }
            | Self::ArtifactSettings { .. }
            | Self::ArtifactEdited { .. }
            | Self::RegenerateArtifact { .. }
            | Self::ArtifactRegenerated { .. }
            | Self::Cursor { .. }
            | Self::Ime { .. }
            | Self::Presence { .. }
            | Self::ChildReplaced { .. }
            | Self::ChildView { .. }
            | Self::CopyText { .. }
            | Self::PasteText { .. }
            | Self::AspectRatio { .. }
            | Self::IntrinsicSize { .. }
            | Self::Performance { .. }
            | Self::Panes { .. }
            | Self::ShowPane { .. }
            | Self::PanesArranged { .. }
            | Self::ClosePane { .. } => {}
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
        BlockQuery::Roots | BlockQuery::Detached => {}
    }
}
