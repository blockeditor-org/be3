//! What a host and an app say to each other on a `Wire`, in the host's terms: `Outbound` and the
//! calls it carries, `Heard`, and visiting the block ids that cross the protocol (`BlockIds`).
use be_sdk_api::app::EditorEvent;
use be_sdk_api::clipboard::ClipBody;
use be_sdk_api::{
    AudioStatus, BlockIdRole, BlockLocation, Catalog, ChildStatus, EditorInstanceId, Failure,
    Fonts, Hello, InputBatch, LayoutCommit, Opening, PickedBlock, Preferences, ScreenId,
    ScreenLayout, ScreenRequest, WebViewEvent, WebViewId,
    embed::{debug::debug::Call as DebugCall, shell::Call as ShellCall},
    shell_types::{ChildCreation, ClipOffer, DragStart, Focus, ShellMessage},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub enum Outbound {
    Open(Box<Opening>),
    Close(EditorInstanceId),
    /// What happened around an editor (`editor.hear`): queued and sent in order.
    Event(EditorInstanceId, EditorEvent),
    Editor(EditorInstanceId, EditorCall),
    App(AppCall),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppCall {
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
pub enum EditorCall {
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
    CommitCreation,
    RegenerateArtifact(Vec<u8>),
    RenderClip {
        request_id: u64,
        serial: u64,
        format: String,
    },
    Dock(be_sdk_api::DockUpdate),
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorAnswer {
    Created(Result<[u8; 16], Failure>),
    Regenerated(Result<(), Failure>),
    Rendered {
        request_id: u64,
        body: Option<ClipBody>,
    },
}

impl From<AppCall> for Outbound {
    fn from(call: AppCall) -> Self {
        Self::App(call)
    }
}

impl Outbound {
    pub fn instance(&self) -> Option<EditorInstanceId> {
        match self {
            Self::Open(opening) => Some(opening.instance),
            Self::Close(instance) | Self::Event(instance, _) | Self::Editor(instance, _) => {
                Some(*instance)
            }
            Self::App(_) => None,
        }
    }

    pub fn closes(&self) -> bool {
        matches!(self, Self::Close(_))
    }
}

#[derive(Debug)]
pub enum Heard {
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
    /// The app opened a published watch: its editor, what opens it, and the watch's own op.
    Watch(EditorInstanceId, be_sdk_api::ops::MethodId, be_sdk_api::ops::OpId),
    Unwatch(EditorInstanceId, be_sdk_api::ops::MethodId),
    Published(EditorInstanceId, be_sdk_api::ops::Started),
    Debug(EditorInstanceId, DebugCall),
    Ended(be_sdk_api::ops::OpId),
    Service(crate::wire::Service),
    Network(EditorInstanceId, be_sdk_api::embed::network::Call),
    Withdrawn(be_sdk_api::ops::OpId),
}

pub fn instance_of(call: &ShellCall) -> Option<EditorInstanceId> {
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

pub type Visit<'a> = dyn FnMut(EditorInstanceId, BlockIdRole, &mut [u8; 16]) + 'a;

pub trait BlockIds {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>);
}

impl BlockIds for LayoutCommit {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        LayoutCommit::visit_block_ids(self, visit);
    }
}

/// Visits the blocks a clip item names, which cross the protocol in its
/// `refs` whatever its format: copied, dragged, rendered or pasted.
pub fn visit_refs(instance: EditorInstanceId, refs: &mut [Uuid], visit: &mut Visit<'_>) {
    for block in refs {
        let mut id = block.into_bytes();
        visit(instance, BlockIdRole::Existing, &mut id);
        *block = Uuid::from_bytes(id);
    }
}

/// The items pasted into an instance's screen, whose refs cross into it.
pub struct Pasted<'a> {
    pub instance: EditorInstanceId,
    pub items: Vec<&'a mut be_sdk_api::ClipItem>,
}

impl BlockIds for Pasted<'_> {
    fn visit_block_ids(&mut self, visit: &mut Visit<'_>) {
        for item in &mut self.items {
            visit_refs(self.instance, &mut item.refs, visit);
        }
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
            Self::Event(instance, EditorEvent::FocusChanged(focus)) => {
                focus_ids(*instance, focus, visit);
            }
            Self::Event(instance, EditorEvent::Drag(Some(drag))) => {
                for item in &mut drag.items {
                    visit_refs(*instance, &mut item.refs, visit);
                }
            }
            Self::Close(_) | Self::Event(..) | Self::Editor(..) | Self::App(_) => {}
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
                ShellMessage::DragBlock(DragStart { block_id, .. }) => {
                    visit(instance, BlockIdRole::Existing, block_id)
                }
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
                        if let Some(body) = &mut item.body {
                            visit_refs(instance, &mut body.refs, visit);
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
            (
                instance,
                EditorAnswer::Rendered {
                    body: Some(body), ..
                },
            ) => visit_refs(*instance, &mut body.refs, visit),
            _ => {}
        }
    }
}
