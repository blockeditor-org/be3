use be_sdk_api::clipboard::{Blocks, ClipFormat as _, formats};
use be_sdk_api::{
    AudioStatus, BarAction, BlockIdRole, BlockLocation, Catalog, ChildId, ChildStatus,
    EditorInstanceId, Failure, Fonts, Hello, InputBatch, LayoutCommit, Opening, PickedBlock,
    Preferences, Rect, ScreenId, ScreenLayout, ScreenRequest, Size, ViewId, WebViewEvent,
    WebViewId,
    embed::{debug::debug::Call as DebugCall, shell::Call as ShellCall},
    shell_types::{BlockAct, ChildCreation, ClipOffer, DragOver, DragStart, Focus, ShellMessage},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outbound {
    Open(Box<Opening>),
    Close(EditorInstanceId),
    Editor(EditorInstanceId, EditorCall),
    App(AppCall),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AppCall {
    Preferences(Preferences),
    UtcOffset(i32),
    Fonts(Fonts),
    Screens(Vec<ScreenRequest>),
    Configure(u64),
    Input(InputBatch),
    Draw {
        now_micros: u64,
        wall_micros: u64,
        seen: bool,
    },
    BlockTypes(Catalog),
    DebugTemplates(Vec<be_sdk_api::DebugTemplate>),
    Describe(bool),
    RunAction {
        screen: ScreenId,
        id: String,
    },
    NodeAction {
        screen: ScreenId,
        request: be_sdk_api::accessibility::ActionRequest,
    },
    Paintings,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum EditorCall {
    MenuPick {
        view: ViewId,
        id: String,
        anchor: Option<beui::Rect>,
    },
    Dismiss,
    ChildBar {
        view: ViewId,
        child: ChildId,
        action: BarAction,
    },
    ChildFocused {
        view: ViewId,
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
    ChildStatuses(Vec<ChildStatus>),
    Watched {
        open: be_sdk_api::ops::MethodId,
        value: std::sync::Arc<Vec<u8>>,
    },
    AudioStatus(AudioStatus),
    WebViewEvent {
        web_view: WebViewId,
        event: WebViewEvent,
    },
    ArtifactSettings(Vec<u8>),
    CommitCreation,
    RegenerateArtifact(Vec<u8>),
    ReplaceChild {
        request_id: u64,
        old: [u8; 16],
        new: [u8; 16],
    },
    RenderClip {
        request_id: u64,
        serial: u64,
        format: String,
    },
    ShowDialog {
        block: [u8; 16],
        dialog: be_sdk_api::ShellDialog,
    },
    Dock(be_sdk_api::DockUpdate),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum EditorAnswer {
    Created(Result<[u8; 16], Failure>),
    Regenerated(Result<(), Failure>),
    Replaced {
        request_id: u64,
        replaced: bool,
    },
    Rendered {
        request_id: u64,
        data: Option<Vec<u8>>,
    },
}

impl From<AppCall> for Outbound {
    fn from(call: AppCall) -> Self {
        Self::App(call)
    }
}

impl Outbound {
    pub(super) fn instance(&self) -> Option<EditorInstanceId> {
        match self {
            Self::Open(opening) => Some(opening.instance),
            Self::Close(instance) | Self::Editor(instance, _) => Some(*instance),
            Self::App(_) => None,
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
    LaidOut(Box<LayoutCommit>),
    Presented(be_sdk_api::PresentCommit),
    ShutDown,
    Unanswered(String),
    Shell(ShellCall),
    Answer(EditorInstanceId, EditorAnswer),
    Paintings(Vec<u8>),
    Blocks(EditorInstanceId, be_sdk_api::embed::blocks::Call),
    Watch(EditorInstanceId, be_sdk_api::ops::MethodId),
    Unwatch(EditorInstanceId, be_sdk_api::ops::MethodId),
    Published(EditorInstanceId, be_sdk_api::ops::Started),
    Debug(EditorInstanceId, DebugCall),
    Ended(be_sdk_api::ops::OpId),
    Service(super::wire::Service),
    Network(EditorInstanceId, be_sdk_api::embed::network::Call),
    Withdrawn(be_sdk_api::ops::OpId),
}

pub(super) fn instance_of(call: &ShellCall) -> Option<EditorInstanceId> {
    match call {
        ShellCall::Tell { instance, .. }
        | ShellCall::ChildStatusesWatchNew { instance, .. }
        | ShellCall::AudioStatusWatchNew { instance, .. }
        | ShellCall::WebViewNew { instance, .. } => Some(*instance),
        ShellCall::ChildStatusesWatchNext { .. }
        | ShellCall::BlockTypesWatchNew { .. }
        | ShellCall::BlockTypesWatchNext { .. }
        | ShellCall::AudioStatusWatchNext { .. }
        | ShellCall::WebViewLoad { .. }
        | ShellCall::WebViewReload { .. }
        | ShellCall::WebViewFocusHost { .. }
        | ShellCall::WebViewNextEvents { .. } => None,
    }
}

pub(super) type Visit<'a> = dyn FnMut(EditorInstanceId, BlockIdRole, &mut [u8; 16]) + 'a;

pub(super) trait BlockIds {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>);
}

impl BlockIds for LayoutCommit {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        LayoutCommit::visit_block_ids(self, visit);
    }
}

/// Visits the ids of a `be3/blocks` item, which cross the protocol inside
/// clips copied and dragged.
fn visit_blocks(
    instance: EditorInstanceId,
    format: &str,
    data: &mut Vec<u8>,
    visit: &mut Visit<'_>,
) {
    if format != formats::BLOCKS {
        return;
    }
    let Some(mut blocks) =
        Blocks::decode(&be_sdk_api::ClipItem::new(formats::BLOCKS, data.clone()))
    else {
        return;
    };
    for block in &mut blocks.blocks {
        let mut id = block.id.into_bytes();
        visit(instance, BlockIdRole::Existing, &mut id);
        block.id = Uuid::from_bytes(id);
    }
    *data = blocks.encode().data;
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
                for item in &mut drag.items {
                    visit_blocks(*instance, &item.format, &mut item.data, visit);
                }
            }
            Self::Editor(instance, EditorCall::ReplaceChild { old, new, .. }) => {
                visit(*instance, BlockIdRole::Existing, old);
                visit(*instance, BlockIdRole::Existing, new);
            }
            Self::Close(_) | Self::Editor(..) | Self::App(_) => {}
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
        let Self::Tell { messages, .. } = self else {
            return;
        };
        for message in messages {
            match message {
                ShellMessage::DragBlock(DragStart { block_id, .. })
                | ShellMessage::Act(BlockAct {
                    block: block_id, ..
                }) => visit(instance, BlockIdRole::Existing, block_id),
                ShellMessage::Focused(focus) => focus_ids(instance, focus, visit),
                ShellMessage::CommitChild(ChildCreation {
                    parent: BlockLocation::Block(block_id),
                    ..
                })
                | ShellMessage::Picked(Ok(Some(PickedBlock { block_id, .. }))) => {
                    visit(instance, BlockIdRole::Existing, block_id)
                }
                ShellMessage::Copy(ClipOffer { items, .. }) => {
                    for item in items {
                        if let Some(data) = &mut item.data {
                            visit_blocks(instance, &item.format, data, visit);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

impl BlockIds for (EditorInstanceId, EditorAnswer) {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        match self {
            (instance, EditorAnswer::Created(Ok(block_id))) => {
                visit(*instance, BlockIdRole::Existing, block_id)
            }
            _ => {}
        }
    }
}
