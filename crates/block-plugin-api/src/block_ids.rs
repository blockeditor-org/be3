use crate::{
    AccessListing, BlockCommand, BlockImport, BlockLocation, BlockPick, BlockQuery, ChildContent,
    ChildPlacements, CreationOutcome, EditorInstanceId, EditorMessage, HostReply, HostRequest,
    Message, ShellDialog, VersionCommand,
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
            | Self::BlockTypes(_)
            | Self::ChildStatuses(_)
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
            | Self::DragBlock { block_id, .. }
            | Self::DragOver { block_id, .. }
            | Self::Define { block_id, .. }
            | Self::VersionStatus { block_id, .. }
            | Self::Head { block_id, .. }
            | Self::Replica { block_id, .. } => existing(block_id),
            Self::OpenArtifact {
                block_id, source, ..
            } => {
                existing(block_id);
                existing(source);
            }
            Self::ShowDialog {
                block_id, dialog, ..
            } => {
                existing(block_id);
                if let ShellDialog::Rename {
                    within: Some(within),
                    ..
                } = dialog
                {
                    existing(within);
                }
            }
            Self::SetAccess { block_id, .. } => existing(block_id),
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
            Self::PickRequested {
                filter,
                parent,
                within,
                ..
            } => {
                filter.excluded.iter_mut().for_each(&mut existing);
                if let Some(place) = &mut filter.place {
                    location(place, &mut existing);
                }
                location(parent, &mut existing);
                if let Some(within) = within {
                    existing(within);
                }
            }
            Self::CommitChild { parent, .. } => location(parent, &mut existing),
            Self::PickAnswered { answer, .. } => match answer {
                BlockPick::Chosen { block_id, .. } => existing(block_id),
                BlockPick::Cancelled | BlockPick::Failed(_) => {}
            },
            Self::Request { request, .. } => match request {
                HostRequest::PickBlock(filter) => {
                    filter.excluded.iter_mut().for_each(&mut existing);
                    if let Some(place) = &mut filter.place {
                        location(place, &mut existing);
                    }
                }
                HostRequest::ListRevisions(block_id)
                | HostRequest::PreviewRevision { block_id, .. }
                | HostRequest::RestoreRevision { block_id, .. }
                | HostRequest::ReleaseRevision(block_id)
                | HostRequest::ListAccess(block_id)
                | HostRequest::ExportBlock(block_id)
                | HostRequest::ReadPlainFile(block_id)
                | HostRequest::ContactLink { block_id, .. } => existing(block_id),
                HostRequest::ImportBlocks(parent) => location(parent, &mut existing),
                HostRequest::CopyBlocks { source, parent } => {
                    existing(source);
                    location(parent, &mut existing);
                }
                HostRequest::PickFile(_)
                | HostRequest::SaveFile(_)
                | HostRequest::Fetch(_)
                | HostRequest::ListData
                | HostRequest::ReadData(_) => {}
            },
            Self::Replied { reply, .. } => match reply {
                HostReply::AccessListed(AccessListing::Listed { grants, .. }) => {
                    for grant in grants {
                        grant.source.iter_mut().for_each(&mut existing);
                    }
                }
                HostReply::RevisionPreviewed(Ok(block_id))
                | HostReply::BlockPicked(BlockPick::Chosen { block_id, .. })
                | HostReply::BlocksImported(BlockImport::Imported { block_id, .. }) => {
                    existing(block_id)
                }
                HostReply::BlockPicked(BlockPick::Cancelled | BlockPick::Failed(_))
                | HostReply::RevisionsListed(_)
                | HostReply::RevisionPreviewed(Err(_))
                | HostReply::RevisionRestored(_)
                | HostReply::RevisionReleased
                | HostReply::FilePicked(_)
                | HostReply::FileSaved(_)
                | HostReply::PlainFileRead(_)
                | HostReply::Fetched(_)
                | HostReply::DataListed(_)
                | HostReply::DataRead(_)
                | HostReply::AccessListed(AccessListing::Failed(_))
                | HostReply::ContactLinkMade(_)
                | HostReply::BlocksImported(BlockImport::Cancelled | BlockImport::Failed(_)) => {}
            },
            Self::CreationBlock { outcome, .. } => match outcome {
                CreationOutcome::Created(block_id) => existing(block_id),
                CreationOutcome::Failed(_) => {}
            },
            Self::ArtifactDescribed { .. } => {}
            Self::WatchArtifacts { blocks, .. }
            | Self::WatchHistory { blocks, .. }
            | Self::WatchHeads { blocks, .. } => {
                blocks.iter_mut().for_each(existing);
            }
            Self::ArtifactStates { states, .. } => {
                for state in states {
                    existing(&mut state.block_id);
                    existing(&mut state.source);
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
            Self::EditabilityChanged { .. }
            | Self::ViewChanged { .. }
            | Self::ChangeView { .. }
            | Self::Present { .. }
            | Self::PresentingChanged { .. }
            | Self::Resized { .. }
            | Self::LeaveFrame { .. }
            | Self::Stopped { .. }
            | Self::BarAction { .. }
            | Self::Close { .. }
            | Self::DragLeft { .. }
            | Self::FileDrop { .. }
            | Self::FileDropLeft { .. }
            | Self::DragAccepted { .. }
            | Self::AudioStatus { .. }
            | Self::PlayAudio { .. }
            | Self::WebViewCommand { .. }
            | Self::ShowPanel { .. }
            | Self::WatchHostValue { .. }
            | Self::HostValue { .. }
            | Self::HostAction { .. }
            | Self::WebViewEvent { .. }
            | Self::OpenCreation { .. }
            | Self::CreationReady { .. }
            | Self::CommitCreation { .. }
            | Self::ArtifactSettings { .. }
            | Self::ArtifactEdited { .. }
            | Self::RegenerateArtifact { .. }
            | Self::ArtifactRegenerated { .. }
            | Self::Presence { .. }
            | Self::ChildReplaced { .. }
            | Self::ChildBar { .. }
            | Self::ChildFocused { .. }
            | Self::CopyText { .. }
            | Self::CopyImage { .. }
            | Self::Paste { .. }
            | Self::AspectRatio { .. }
            | Self::IntrinsicSize { .. }
            | Self::Performance { .. }
            | Self::Menu { .. }
            | Self::MenuPick { .. }
            | Self::ChildMenuPick { .. } => {}
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
        | BlockQuery::Block(block_id)
        | BlockQuery::OpenSettings(block_id) => visit(block_id),
        BlockQuery::Roots | BlockQuery::Detached | BlockQuery::Shared => {}
    }
}
