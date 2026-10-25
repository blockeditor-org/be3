use block_plugin_api::{
    ArtifactDescription, AudioStatus, BarAction, BlockIdRole, BlockLocation, BlockPick, Catalog,
    ChildId, ChildStatus, EditorInstanceId, EditorRegion, Failure, Fonts, FrameCommit, Hello,
    InputBatch, Opening, PickedBlock, Preferences, Rect, ScreenId, ScreenLayout, ScreenRequest,
    Size, WebViewEvent, WebViewId,
    embed::{debug::Call as DebugCall, shell::Call as ShellCall},
    shell_types::{DragOver, FileDrop, Focus, PickRequest},
};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outbound {
    Open(Box<Opening>),
    Close(EditorInstanceId),
    Editor(EditorInstanceId, EditorCall),
    Plugin(PluginCall),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum PluginCall {
    Preferences(Preferences),
    UtcOffset(i32),
    Fonts(Fonts),
    Screens(Vec<ScreenRequest>),
    Configure(u64),
    Input(InputBatch),
    Draw {
        now_micros: u64,
        seen: bool,
    },
    BlockTypes(Catalog),
    DebugTemplates(Vec<block_plugin_api::DebugTemplate>),
    Describe(bool),
    RunAction {
        screen: ScreenId,
        id: String,
    },
    NodeAction {
        screen: ScreenId,
        request: block_plugin_api::accessibility::ActionRequest,
    },
    Paintings,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum EditorCall {
    MenuPick(String),
    ChildBar {
        region: EditorRegion,
        child: ChildId,
        action: BarAction,
    },
    ChildFocused {
        region: EditorRegion,
        child: ChildId,
    },
    SetEditable(bool),
    ViewChanged {
        view: Rect,
        scale: f32,
    },
    PresentingChanged(bool),
    Presence(bool),
    FocusChanged(Focus),
    Resized(Size),
    DragOver(DragOver),
    DragLeft,
    FileDrop(FileDrop),
    FileDropLeft,
    ChildStatuses(Vec<ChildStatus>),
    Pick(PickRequest),
    HostValue {
        key: String,
        value: Vec<u8>,
    },
    AudioStatus(AudioStatus),
    WebViewEvent {
        web_view: WebViewId,
        event: WebViewEvent,
    },
    WithdrawPick(u64),
    ArtifactSettings(Vec<u8>),
    CommitCreation,
    RegenerateArtifact(Vec<u8>),
    ReplaceChild {
        request_id: u64,
        old: [u8; 16],
        new: [u8; 16],
    },
    ShowDialog {
        block: [u8; 16],
        dialog: block_plugin_api::ShellDialog,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum EditorAnswer {
    Created(Result<[u8; 16], Failure>),
    Regenerated(Result<(), Failure>),
    Replaced { request_id: u64, replaced: bool },
    Picked { pick: u64, answer: BlockPick },
}

impl From<PluginCall> for Outbound {
    fn from(call: PluginCall) -> Self {
        Self::Plugin(call)
    }
}

impl Outbound {
    pub(super) fn instance(&self) -> Option<EditorInstanceId> {
        match self {
            Self::Open(opening) => Some(opening.instance()),
            Self::Close(instance) | Self::Editor(instance, _) => Some(*instance),
            Self::Plugin(_) => None,
        }
    }

    pub(super) fn closes(&self) -> bool {
        matches!(self, Self::Close(_))
    }
}

#[derive(Debug)]
pub(super) enum Heard {
    Hello(Hello),
    Layout(ScreenLayout),
    FrameNeeded,
    MissingCharacters(Vec<char>),
    Frame(Box<FrameCommit>),
    ShutDown,
    Unanswered(String),
    Shell(ShellCall),
    Answer(EditorInstanceId, EditorAnswer),
    Paintings(Vec<u8>),
    Blocks(EditorInstanceId, block_plugin_api::embed::blocks::Call),
    HostValues(EditorInstanceId, block_plugin_api::embed::host_values::Call),
    Debug(EditorInstanceId, DebugCall),
    Ended(block_plugin_api::ops::OpId),
    Service(super::wire::Service),
    Withdrawn(block_plugin_api::ops::OpId),
}

pub(super) fn instance_of(call: &ShellCall) -> Option<EditorInstanceId> {
    match call {
        ShellCall::Menu { instance, .. }
        | ShellCall::RunBarAction { instance, .. }
        | ShellCall::ChildMenuPick { instance, .. }
        | ShellCall::CopyText { instance, .. }
        | ShellCall::CopyImage { instance, .. }
        | ShellCall::Paste { instance }
        | ShellCall::DragBlock { instance, .. }
        | ShellCall::DragAccepted { instance, .. }
        | ShellCall::AspectRatio { instance, .. }
        | ShellCall::IntrinsicSize { instance, .. }
        | ShellCall::Act { instance, .. }
        | ShellCall::LeaveFrame { instance }
        | ShellCall::RegionStopped { instance, .. }
        | ShellCall::Present { instance, .. }
        | ShellCall::ChangeView { instance, .. }
        | ShellCall::Focused { instance, .. }
        | ShellCall::CreationReady { instance, .. }
        | ShellCall::CommitChild { instance, .. }
        | ShellCall::ArtifactDescribed { instance, .. }
        | ShellCall::ArtifactEdited { instance, .. }
        | ShellCall::ChildStatusesWatchNew { instance, .. }
        | ShellCall::PlayAudio { instance, .. }
        | ShellCall::AudioStatusWatchNew { instance, .. }
        | ShellCall::WebViewNew { instance, .. } => Some(*instance),
        ShellCall::ChildStatusesWatchNext { .. }
        | ShellCall::BlockTypesWatchNew { .. }
        | ShellCall::BlockTypesWatchNext { .. }
        | ShellCall::AudioStatusWatchNext { .. }
        | ShellCall::WebViewLoad { .. }
        | ShellCall::WebViewReload { .. }
        | ShellCall::WebViewFocusApp { .. }
        | ShellCall::WebViewNextEvents { .. } => None,
    }
}

pub(super) type Visit<'a> = dyn FnMut(EditorInstanceId, BlockIdRole, &mut [u8; 16]) + 'a;

pub(super) trait BlockIds {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>);
}

impl BlockIds for FrameCommit {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        FrameCommit::visit_block_ids(self, visit);
    }
}

fn focus_ids(instance: EditorInstanceId, focus: &mut Focus, visit: &mut Visit<'_>) {
    if let Some(block_id) = &mut focus.block_id {
        visit(instance, BlockIdRole::Existing, block_id);
    }
    for block_id in &mut focus.via {
        visit(instance, BlockIdRole::Existing, block_id);
    }
}

impl BlockIds for Outbound {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        match self {
            Self::Open(opening) => opening.visit_block_ids(visit),
            Self::Editor(instance, EditorCall::FocusChanged(focus)) => {
                focus_ids(*instance, focus, visit);
            }
            Self::Editor(instance, EditorCall::DragOver(drag)) => {
                visit(*instance, BlockIdRole::Existing, &mut drag.block_id);
            }
            Self::Editor(instance, EditorCall::Pick(request)) => {
                let mut existing = |id: &mut [u8; 16]| visit(*instance, BlockIdRole::Existing, id);
                request.filter.excluded.iter_mut().for_each(&mut existing);
                if let Some(BlockLocation::Block(place)) = &mut request.filter.place {
                    existing(place);
                }
                if let BlockLocation::Block(parent) = &mut request.parent {
                    existing(parent);
                }
                request.within.iter_mut().for_each(existing);
            }
            Self::Editor(instance, EditorCall::ReplaceChild { old, new, .. }) => {
                visit(*instance, BlockIdRole::Existing, old);
                visit(*instance, BlockIdRole::Existing, new);
            }
            Self::Close(_) | Self::Editor(..) | Self::Plugin(_) => {}
        }
    }
}

impl BlockIds for (EditorInstanceId, DebugCall) {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        if let (instance, DebugCall::SimulateAccess { block, .. }) = self {
            visit(*instance, BlockIdRole::Existing, block);
        }
    }
}

impl BlockIds for ShellCall {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        let Some(instance) = instance_of(self) else {
            return;
        };
        match self {
            Self::DragBlock { block_id, .. }
            | Self::Act {
                block: block_id, ..
            } => visit(instance, BlockIdRole::Existing, block_id),
            Self::Focused { focus, .. } => focus_ids(instance, focus, visit),
            Self::CommitChild {
                parent: BlockLocation::Block(block_id),
                ..
            } => visit(instance, BlockIdRole::Existing, block_id),
            Self::ArtifactDescribed {
                description: ArtifactDescription::Described { source, .. },
                ..
            } => visit(instance, BlockIdRole::Existing, source),
            _ => {}
        }
    }
}

impl BlockIds for (EditorInstanceId, EditorAnswer) {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        match self {
            (instance, EditorAnswer::Created(Ok(block_id)))
            | (
                instance,
                EditorAnswer::Picked {
                    answer: Ok(Some(PickedBlock { block_id, .. })),
                    ..
                },
            ) => visit(*instance, BlockIdRole::Existing, block_id),
            _ => {}
        }
    }
}
