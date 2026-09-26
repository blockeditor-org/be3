use be_block::presence::{PresenceKind, UserActive, pick_free_color};
use block_plugin_api::{
    ArtifactDescription, ChildId, ChildPlacement, ChildPlacements, ChildRect, ChildStatus,
    CreationOutcome, CursorIcon, EditorInstanceId, EditorMessage, EditorRegion, FrameChrome,
    FrameReport, FrameSpec, HostReply, ImeArea, ImeInput, InputEvent, Key, MAX_CHILDREN,
    MAX_COLLECTION_ITEMS, Message, Occluder, PointerButton, RegionSize, ScreenPlacement,
    ScreenRequest, Size, ViewChange, ViewportMetrics, WebViewEvent, WheelUnit,
};
use block_ui::BlockCatalog;
use std::{collections::HashMap, marker::PhantomData, rc::Rc};
use uuid::Uuid;

use crate::beui_frame::{BeuiFrame, FrameBar};
use crate::{EditorHost, Waker, beui_frame, host::BlockDrag};

const WHEEL_LINE: f32 = 40.0;
const WHEEL_PAGE: f32 = 400.0;

pub(crate) struct EditorSession {
    app: Box<dyn AppUi>,
    beui: HashMap<EditorRegion, BeuiRegion>,
    instance: EditorInstanceId,
    regions: HashMap<EditorRegion, RegionState>,
    host: EditorHost,
    own_block: Option<Uuid>,
    drag: Option<(EditorRegion, BlockDrag)>,
    files: Option<(EditorRegion, crate::host::FileDrop)>,
    intrinsic: Option<beui::Vec2>,
    aspect_ratio: Option<f32>,
    creating: bool,
    leaving: bool,
    created: Option<CreationOutcome>,
    artifact: Option<ArtifactState>,
    copied: Vec<String>,
    paste_requested: bool,
    replacements: Vec<(u64, bool)>,
    generation: u64,
}

struct ArtifactState {
    data: Vec<u8>,
    draft: Vec<u8>,
    described: Option<Vec<u8>>,
    edited: bool,
    regenerating: bool,
}

impl ArtifactState {
    fn new(data: Vec<u8>) -> Self {
        Self {
            draft: data.clone(),
            data,
            described: None,
            edited: false,
            regenerating: false,
        }
    }

    fn settings(&mut self, data: Vec<u8>) {
        self.draft.clone_from(&data);
        self.data = data;
        self.edited = false;
    }
}

struct BeuiRegion {
    context: beui::Context,
    events: Vec<beui::Event>,
    modifiers: beui::Modifiers,
    pointer: beui::Pos2,
    emulated_touch: bool,
    chrome: Option<BeuiFrame>,
}

impl BeuiRegion {
    fn new() -> Self {
        Self {
            context: beui::Context::new(),
            events: Vec::new(),
            modifiers: beui::Modifiers::NONE,
            pointer: beui::Pos2::ZERO,
            emulated_touch: false,
            chrome: None,
        }
    }

    fn emulate_touch(&mut self, phase: beui::TouchPhase) {
        self.events.push(beui::Event::Touch {
            id: beui::TouchId {
                device: 0,
                finger: 0,
            },
            phase,
            pos: self.pointer,
            force: None,
        });
    }
}

#[derive(Default)]
struct RegionState {
    placement: Option<ScreenPlacement>,
    metrics: Option<ViewportMetrics>,
    frame: Option<FrameSpec>,
    used: Option<beui::Vec2>,
    reported: Option<beui::Vec2>,
    report: Option<FrameReport>,
    reported_frame: Option<FrameReport>,
    cursor: CursorIcon,
    reported_cursor: Option<CursorIcon>,
    ime: Option<ImeArea>,
    reported_ime: Option<Option<ImeArea>>,
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    reported_children: Option<(Vec<ChildPlacement>, Vec<Occluder>)>,
}

trait AppUi {
    fn editor(&self) -> Option<crate::Editor>;
    fn view(&mut self) -> beui::NodeId;
    fn update(&mut self);
    fn after_layout(&mut self, document: &beui::Document);
    fn creation(&mut self, context: &beui::Context, rect: beui::Rect);
    fn preview(&mut self, context: &beui::Context, rect: beui::Rect);
    fn artifact_settings(&mut self, context: &beui::Context, rect: beui::Rect, draft: &mut Vec<u8>);
    fn connect(&mut self, host: EditorHost, block_id: Uuid);
    fn connect_creation(&mut self, host: EditorHost);
    fn create_block(&mut self) -> Result<Uuid, String>;
    fn connect_artifact(&mut self, host: EditorHost, artifact: crate::Artifact);
    fn describe_artifact(&mut self, data: &[u8]) -> ArtifactDescription;
    fn regenerate_artifact(&mut self, data: &[u8]);
    fn poll_artifact(&mut self) -> Option<Result<(), String>>;
    fn intrinsic_size(&mut self) -> Option<beui::Vec2>;
    fn set_intrinsic_size(&mut self, size: beui::Vec2);
    fn aspect_ratio(&mut self) -> Option<f32>;
    fn presence_visible(&mut self, visible: bool);
    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool;
}

struct BeuiHolder<A: crate::BeuiApp> {
    editor: Option<crate::Editor>,
    preview: Option<crate::Editor>,
    preview_document: Option<beui::Document>,
    creation: Option<crate::Creation>,
    dialog: Option<beui::Document>,
    artifacts: Option<crate::Artifacts>,
    settings: Option<beui::Document>,
    app: PhantomData<A>,
}

impl<A: crate::BeuiApp> BeuiHolder<A> {
    fn new() -> Self {
        Self {
            editor: None,
            preview: None,
            preview_document: None,
            creation: None,
            dialog: None,
            artifacts: None,
            settings: None,
            app: PhantomData,
        }
    }
}

impl<A: crate::BeuiApp> AppUi for BeuiHolder<A> {
    fn editor(&self) -> Option<crate::Editor> {
        self.editor.clone()
    }

    fn view(&mut self) -> beui::NodeId {
        let editor = self
            .editor
            .clone()
            .expect("connect is called before the view is built");
        A::view(editor)
    }

    fn update(&mut self) {
        if let Some(editor) = &self.editor {
            editor.begin_frame();
        }
    }

    fn after_layout(&mut self, document: &beui::Document) {
        if let Some(editor) = &self.editor {
            editor.end_frame(document);
        }
    }

    fn preview(&mut self, context: &beui::Context, rect: beui::Rect) {
        let Some(editor) = self.preview.clone() else {
            return;
        };
        let document = self.preview_document.get_or_insert_with(|| {
            let built = editor.clone();
            beui::reactive::build(move || A::preview_view(built))
        });
        let begun = editor.clone();
        beui::reactive::with_reactive_scope(document, move || begun.begin_frame());
        document.show(context, rect);
        editor.end_frame(document);
    }

    fn creation(&mut self, context: &beui::Context, rect: beui::Rect) {
        let (Some(creation), Some(dialog)) = (self.creation.as_ref(), self.dialog.as_mut()) else {
            return;
        };
        let creation = creation.clone();
        beui::reactive::with_reactive_scope(dialog, move || creation.begin_frame());
        dialog.show(context, rect);
    }

    fn connect(&mut self, host: EditorHost, block_id: Uuid) {
        self.editor = Some(crate::Editor::new(host.clone(), block_id));
        self.preview = Some(crate::Editor::new(host, block_id));
        self.preview_document = None;
    }

    fn connect_creation(&mut self, host: EditorHost) {
        let creation = crate::Creation::new(host);
        let built = creation.clone();
        self.dialog = Some(beui::reactive::build(move || A::creation_view(built)));
        self.creation = Some(creation);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let creation = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        A::create_block(creation)
    }

    fn connect_artifact(&mut self, host: EditorHost, artifact: crate::Artifact) {
        let artifacts = crate::Artifacts::new(host, artifact);
        A::connect_artifact(&artifacts);
        self.artifacts = Some(artifacts);
    }

    fn describe_artifact(&mut self, data: &[u8]) -> ArtifactDescription {
        match A::describe_artifact(data) {
            Ok(description) => ArtifactDescription::Described {
                source: description.source.into_bytes(),
                summary: description.summary,
            },
            Err(error) => ArtifactDescription::Unreadable(error),
        }
    }

    fn artifact_settings(
        &mut self,
        context: &beui::Context,
        rect: beui::Rect,
        draft: &mut Vec<u8>,
    ) {
        let Some(artifacts) = self.artifacts.clone() else {
            return;
        };
        let document = self.settings.get_or_insert_with(|| {
            let built = artifacts.clone();
            beui::reactive::build(move || A::artifact_settings_view(built))
        });
        let received = artifacts.clone();
        let data = std::mem::take(draft);
        beui::reactive::with_reactive_scope(document, move || received.receive_settings(&data));
        document.show(context, rect);
        *draft = artifacts
            .take_settings_edit()
            .unwrap_or_else(|| artifacts.settings().get_untracked());
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        if let Some(artifacts) = &self.artifacts {
            artifacts.regenerate(data);
        }
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        self.artifacts.as_ref()?.poll()
    }

    fn intrinsic_size(&mut self) -> Option<beui::Vec2> {
        self.editor
            .as_ref()
            .and_then(crate::Editor::intrinsic_size)
            .or_else(A::intrinsic_size)
    }

    fn set_intrinsic_size(&mut self, size: beui::Vec2) {
        if let Some(editor) = &self.editor {
            editor.report_resize(size);
        }
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        A::aspect_ratio()
    }

    fn presence_visible(&mut self, visible: bool) {
        if let Some(editor) = &self.editor {
            editor.report_presence_visible(visible);
        }
    }

    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|editor| editor.replace_child(old, new))
    }
}

impl EditorSession {
    pub(crate) fn new<A: crate::BeuiApp>(instance: EditorInstanceId, waker: Waker) -> Self {
        Self {
            app: Box::new(BeuiHolder::<A>::new()),
            beui: HashMap::new(),
            instance,
            regions: HashMap::new(),
            host: EditorHost::new(waker),
            own_block: None,
            drag: None,
            files: None,
            intrinsic: None,
            aspect_ratio: None,
            creating: false,
            leaving: false,
            created: None,
            artifact: None,
            copied: Vec::new(),
            paste_requested: false,
            replacements: Vec::new(),
            generation: 0,
        }
    }

    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        self.host.set_block_types(catalog);
    }

    pub(crate) fn set_audio(&self, status: block_plugin_api::AudioStatus) {
        self.host.set_audio(status);
    }

    pub(crate) fn set_client_id(&self, client_id: Uuid) {
        self.host.set_client_id(client_id);
    }

    pub(crate) fn set_account_id(&self, account: Uuid) {
        self.host.set_account_id(account);
    }

    pub(crate) fn set_workspace_id(&self, workspace: Uuid) {
        self.host.set_workspace_id(workspace);
    }

    pub(crate) fn set_blocks(&self, query: crate::BlockQuery, blocks: Vec<crate::BlockInfo>) {
        self.host.set_blocks(query, blocks);
    }

    pub(crate) fn set_editable(&self, editable: bool) {
        self.host.set_editable(editable);
    }

    pub(crate) fn set_block_content(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
        applied: u64,
    ) {
        match self.own_block == Some(block) {
            true => self.host.set_block_content(content_type, bytes, applied),
            false => self
                .host
                .set_content_of(block, content_type, bytes, applied),
        }
    }

    pub(crate) fn set_peers(&self, block: Uuid, peers: Vec<crate::PeerPresence>) {
        let block = (self.own_block != Some(block)).then_some(block);
        self.host.set_peers(block, peers);
    }

    pub(crate) fn push_content_operations(&self, block: Uuid, operations: Vec<(Vec<u8>, bool)>) {
        match self.own_block == Some(block) {
            true => self.host.push_content_operations(operations),
            false => self.host.push_content_operations_of(block, operations),
        }
    }

    pub(crate) fn set_focused_block(&self, focused: crate::host::FocusedBlock) {
        self.host.set_focused_block(focused);
    }

    pub(crate) fn show_block(&self, block_id: Uuid, block_type: Uuid, via: Option<Uuid>) {
        self.host.show_block(block_id, block_type, via);
    }

    pub(crate) fn set_artifacts(&self, states: Vec<crate::host::ArtifactState>) {
        self.host.set_artifacts(states);
    }

    pub(crate) fn set_histories(&self, states: &[block_plugin_api::HistoryState]) {
        self.host.set_histories(states.iter().map(|state| {
            (
                Uuid::from_bytes(state.block_id),
                crate::host::BlockHistory {
                    can_undo: state.can_undo,
                    can_redo: state.can_redo,
                },
            )
        }));
    }

    pub(crate) fn set_view(&self, view: beui::Rect, scale: f32) {
        self.host.set_view(view, scale);
    }

    pub(crate) fn resized(&mut self, size: beui::Vec2) {
        self.app.set_intrinsic_size(size);
    }

    pub(crate) fn presence_visible(&mut self, visible: bool) {
        if self.own_block.is_some() {
            let value = visible.then(|| {
                let peers = self
                    .host
                    .peers_since(None, 0)
                    .map(|(_, peers)| peers)
                    .unwrap_or_default();
                let used = peers
                    .iter()
                    .filter(|peer| peer.kind == UserActive::ID)
                    .filter_map(|peer| serde_json::from_slice::<UserActive>(&peer.value).ok())
                    .map(|user| user.color);
                let color = pick_free_color(used);
                serde_json::to_vec(&UserActive { color }).unwrap_or_default()
            });
            self.host.show_presence(None, UserActive::ID, value);
        }
        self.app.presence_visible(visible);
    }

    pub(crate) fn replace_child(&mut self, request_id: u64, old: Uuid, new: Uuid) {
        let document = self
            .beui
            .get_mut(&EditorRegion::Frame)
            .and_then(|region| region.chrome.as_mut())
            .map(BeuiFrame::document_mut);
        let app = &mut self.app;
        let replaced = match document {
            Some(document) => {
                beui::reactive::with_reactive_scope(document, || app.replace_child(old, new))
            }
            None => app.replace_child(old, new),
        };
        self.replacements.push((request_id, replaced));
    }

    pub(crate) fn set_presenting(&self, presenting: bool) {
        self.host.set_presenting(presenting);
    }

    pub(crate) fn set_drag(&mut self, drag: Option<(EditorRegion, BlockDrag)>) {
        self.drag = drag;
    }

    pub(crate) fn set_files(&mut self, files: Option<(EditorRegion, crate::host::FileDrop)>) {
        self.files = files;
    }

    pub(crate) fn connect(&mut self, block_id: Uuid, block_type: Uuid) {
        self.host.set_block_type(block_type);
        self.own_block = Some(block_id);
        self.app.connect(self.host.clone(), block_id);
    }

    pub(crate) fn connect_creation(&mut self) {
        self.creating = true;
        self.host.set_editable(true);
        self.app.connect_creation(self.host.clone());
    }

    pub(crate) fn connect_artifact(&mut self, block_id: Uuid, block_type: Uuid, data: Vec<u8>) {
        self.artifact = Some(ArtifactState::new(data));
        self.host.set_editable(true);
        self.app.connect_artifact(
            self.host.clone(),
            crate::Artifact {
                block_id,
                block_type,
            },
        );
    }

    pub(crate) fn artifact_settings(&mut self, data: Vec<u8>) {
        if let Some(artifact) = &mut self.artifact {
            artifact.settings(data);
        }
    }

    pub(crate) fn regenerate_artifact(&mut self, data: &[u8]) {
        let Some(artifact) = &mut self.artifact else {
            return;
        };
        artifact.regenerating = true;
        self.app.regenerate_artifact(data);
    }

    pub(crate) fn commit_creation(&mut self) {
        self.created = Some(match self.app.create_block() {
            Ok(block_id) => CreationOutcome::Created(block_id.into_bytes()),
            Err(error) => CreationOutcome::Failed(error),
        });
    }

    pub(crate) fn place(&mut self, placements: &[ScreenPlacement], requests: &[ScreenRequest]) {
        self.regions
            .retain(|region, _| placements.iter().any(|it| it.region == *region));
        for placement in placements {
            let Some(request) = requests
                .iter()
                .find(|request| request.screen == placement.screen)
            else {
                continue;
            };
            let state = self.regions.entry(placement.region).or_default();
            state.placement = Some(*placement);
            state.metrics = Some(request.metrics.clone());
            state.frame = request.frame.clone();
        }
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        let sizes = self.region_sizes();
        if !sizes.is_empty() {
            messages.push(Message::RegionSizes(sizes));
        }
        let frames = self.frame_reports();
        if !frames.is_empty() {
            messages.push(Message::Frames(frames));
        }
        let instance = self.instance;
        for (group, measurements) in self.host.take_performance() {
            messages.push(Message::Editor(EditorMessage::Performance {
                instance,
                group,
                measurements,
            }));
        }
        for (region, cursor) in self.cursors() {
            messages.push(Message::Editor(EditorMessage::Cursor {
                instance,
                region,
                cursor,
            }));
        }
        for (region, area) in self.ime_areas() {
            messages.push(Message::Editor(EditorMessage::Ime {
                instance,
                region,
                area,
            }));
        }
        for command in self.host.take_graph_commands() {
            messages.push(Message::Editor(match command {
                crate::GraphCommand::Create {
                    id,
                    block_type,
                    parent,
                    name,
                    artifact,
                    content,
                } => EditorMessage::CreateBlock {
                    instance,
                    block_id: id.into_bytes(),
                    content_type: block_type.into_bytes(),
                    parent: parent.encode(),
                    name,
                    artifact: artifact.map(|artifact| block_plugin_api::ArtifactSource {
                        source_type: artifact.source_type.into_bytes(),
                        data: artifact.data,
                    }),
                    content: content.map(serde_bytes::ByteBuf::from),
                },
                crate::GraphCommand::SetParent { id, parent } => EditorMessage::SetParent {
                    instance,
                    block_id: id.into_bytes(),
                    parent: parent.encode(),
                },
                crate::GraphCommand::SetName { id, name } => EditorMessage::SetName {
                    instance,
                    block_id: id.into_bytes(),
                    name,
                },
            }));
        }
        if let Some(queries) = self.host.take_block_watch() {
            messages.push(Message::Editor(EditorMessage::WatchBlocks {
                instance,
                queries: queries.into_iter().map(crate::BlockQuery::encode).collect(),
            }));
        }
        for (block, operation) in self.host.take_all_content_operations() {
            let Some(block) = block.or(self.own_block) else {
                continue;
            };
            messages.push(Message::Editor(EditorMessage::Operate {
                instance,
                block_id: block.into_bytes(),
                operation,
            }));
        }
        if let Some(watched) = self.host.take_content_watch() {
            messages.push(Message::Editor(EditorMessage::WatchContent {
                instance,
                blocks: watched
                    .into_iter()
                    .map(|(block, content_type)| block_plugin_api::WatchedContent {
                        block_id: block.into_bytes(),
                        content_type: content_type.into_bytes(),
                    })
                    .collect(),
            }));
        }
        if let Some(accepted) = self.host.take_drag_accepted() {
            messages.push(Message::Editor(EditorMessage::DragAccepted {
                instance,
                accepted,
            }));
        }
        let intrinsic = self.app.intrinsic_size();
        if intrinsic != self.intrinsic {
            self.intrinsic = intrinsic;
            messages.push(Message::Editor(EditorMessage::IntrinsicSize {
                instance,
                size: intrinsic.map(|size| Size {
                    width: size.x,
                    height: size.y,
                }),
            }));
        }
        let aspect_ratio = self.app.aspect_ratio();
        if aspect_ratio != self.aspect_ratio {
            self.aspect_ratio = aspect_ratio;
            messages.push(Message::Editor(EditorMessage::AspectRatio {
                instance,
                ratio: aspect_ratio,
            }));
        }
        if let Some(ready) = self.host.take_creation_ready() {
            messages.push(Message::Editor(EditorMessage::CreationReady {
                instance,
                ready,
            }));
        }
        if let Some(artifact) = &mut self.artifact {
            if artifact.described.as_deref() != Some(artifact.data.as_slice()) {
                artifact.described = Some(artifact.data.clone());
                let description = self.app.describe_artifact(&artifact.data);
                messages.push(Message::Editor(EditorMessage::ArtifactDescribed {
                    instance,
                    description,
                }));
            }
            if artifact.edited {
                artifact.edited = false;
                messages.push(Message::Editor(EditorMessage::ArtifactEdited {
                    instance,
                    data: artifact.draft.clone(),
                }));
            }
            if artifact.regenerating
                && let Some(result) = self.app.poll_artifact()
            {
                artifact.regenerating = false;
                messages.push(Message::Editor(EditorMessage::ArtifactRegenerated {
                    instance,
                    outcome: match result {
                        Ok(()) => block_plugin_api::RegenerationOutcome::Done,
                        Err(error) => block_plugin_api::RegenerationOutcome::Failed(error),
                    },
                }));
            }
        }
        if std::mem::take(&mut self.leaving) {
            messages.push(Message::Editor(EditorMessage::LeaveFrame { instance }));
        }
        for shown in self.host.take_shown_presence() {
            let Some(block) = shown.block.or(self.own_block) else {
                continue;
            };
            messages.push(Message::Editor(EditorMessage::ShowPresence {
                instance,
                block_id: block.into_bytes(),
                kind: shown.kind.into_bytes(),
                value: shown.value.map(serde_bytes::ByteBuf::from),
            }));
        }
        for seeded in self.host.take_seeded_content() {
            let (block_id, content_type, bytes) = (
                seeded.block.into_bytes(),
                seeded.content_type.into_bytes(),
                seeded.bytes,
            );
            messages.push(Message::Editor(match seeded.replace {
                true => EditorMessage::ReplaceContent {
                    instance,
                    block_id,
                    content_type,
                    bytes,
                },
                false => EditorMessage::SeedContent {
                    instance,
                    block_id,
                    content_type,
                    bytes,
                },
            }));
        }
        if let Some(outcome) = self.created.take() {
            messages.push(Message::Editor(EditorMessage::CreationBlock {
                instance,
                outcome,
            }));
        }
        for text in std::mem::take(&mut self.copied) {
            messages.push(Message::Editor(EditorMessage::CopyText { instance, text }));
        }
        if std::mem::take(&mut self.paste_requested) {
            messages.push(Message::Editor(EditorMessage::PasteText { instance }));
        }
        for (request_id, replaced) in std::mem::take(&mut self.replacements) {
            messages.push(Message::Editor(EditorMessage::ChildReplaced {
                instance,
                request_id,
                replaced,
            }));
        }
        for (request_id, request) in self.host.take_requests() {
            messages.push(Message::Editor(EditorMessage::Request {
                instance,
                request_id,
                request,
            }));
        }
        for (block_id, command) in self.host.take_audio_commands() {
            messages.push(Message::Editor(EditorMessage::PlayAudio {
                instance,
                block_id: block_id.into_bytes(),
                command,
            }));
        }
        for (region, rect) in self.host.take_web_view_placements() {
            messages.push(Message::Editor(EditorMessage::WebView {
                instance,
                region,
                rect,
            }));
        }
        for command in self.host.take_web_view_commands() {
            messages.push(Message::Editor(EditorMessage::WebViewCommand {
                instance,
                command,
            }));
        }
        if let Some(grabbed) = self.host.take_cursor_grab() {
            messages.push(Message::Editor(EditorMessage::GrabCursor {
                instance,
                grabbed,
            }));
        }
        for placements in self.children() {
            messages.push(Message::Children(placements));
        }
        self.retain_child_statuses();
        for presenting in self.host.take_present_requests() {
            messages.push(Message::Editor(EditorMessage::Present {
                instance,
                presenting,
            }));
        }
        for change in self.host.take_view_changes() {
            messages.push(Message::Editor(EditorMessage::ChangeView {
                instance,
                change,
            }));
        }
        for (block_id, block_type, via) in self.host.take_opens() {
            messages.push(Message::Editor(EditorMessage::OpenBlock {
                instance,
                block_id: block_id.into_bytes(),
                block_type: block_type.into_bytes(),
                via: via.map(Uuid::into_bytes),
            }));
        }
        for (block_id, block_type) in self.host.take_block_drags() {
            messages.push(Message::Editor(EditorMessage::DragBlock {
                instance,
                block_id: block_id.into_bytes(),
                block_type: block_type.into_bytes(),
            }));
        }
        for (block_id, command) in self.host.take_block_commands() {
            messages.push(Message::Editor(EditorMessage::BlockCommand {
                instance,
                block_id: block_id.into_bytes(),
                command,
            }));
        }
        if let Some(focused) = self.host.take_focus_report() {
            messages.push(Message::Editor(EditorMessage::Focused {
                instance,
                block_id: focused.block_id.map(Uuid::into_bytes),
                block_type: focused.block_type.into_bytes(),
                via: focused.via.into_iter().map(Uuid::into_bytes).collect(),
            }));
        }
        if let Some(blocks) = self.host.take_artifact_watch() {
            messages.push(Message::Editor(EditorMessage::WatchArtifacts {
                instance,
                blocks: blocks.into_iter().map(Uuid::into_bytes).collect(),
            }));
        }
        if let Some(blocks) = self.host.take_history_watch() {
            messages.push(Message::Editor(EditorMessage::WatchHistory {
                instance,
                blocks: blocks.into_iter().map(Uuid::into_bytes).collect(),
            }));
        }
        messages
    }

    fn cursors(&mut self) -> Vec<(EditorRegion, CursorIcon)> {
        let mut cursors = Vec::new();
        for (region, state) in &mut self.regions {
            if state.reported_cursor == Some(state.cursor) {
                continue;
            }
            state.reported_cursor = Some(state.cursor);
            cursors.push((*region, state.cursor));
        }
        cursors
    }

    fn ime_areas(&mut self) -> Vec<(EditorRegion, Option<ImeArea>)> {
        let mut areas = Vec::new();
        for (region, state) in &mut self.regions {
            if state.reported_ime == Some(state.ime) {
                continue;
            }
            state.reported_ime = Some(state.ime);
            areas.push((*region, state.ime));
        }
        areas
    }

    fn frame_reports(&mut self) -> Vec<FrameReport> {
        let mut reports = Vec::new();
        for state in self.regions.values_mut() {
            let Some(report) = &state.report else {
                continue;
            };
            if state.reported_frame.as_ref() == Some(report) {
                continue;
            }
            state.reported_frame = Some(report.clone());
            reports.push(report.clone());
        }
        reports
    }

    fn region_sizes(&mut self) -> Vec<RegionSize> {
        let mut sizes = Vec::new();
        for state in self.regions.values_mut() {
            let (Some(placement), Some(used)) = (state.placement, state.used) else {
                continue;
            };
            if state.reported == Some(used) {
                continue;
            }
            state.reported = Some(used);
            sizes.push(RegionSize {
                screen: placement.screen,
                logical_width: used.x,
                logical_height: used.y,
            });
        }
        sizes
    }

    fn used(&mut self, region: EditorRegion, content: beui::Rect) {
        let origin = self.rect(region).min;
        let Some(state) = self.regions.get_mut(&region) else {
            return;
        };
        let used = (content.max - origin).max(beui::Vec2::ZERO);
        state.used = Some(beui::vec2(used.x.round(), used.y.round()));
    }

    pub(crate) fn replied(&self, request_id: u64, reply: HostReply) {
        self.host.set_reply(request_id, reply);
    }

    pub(crate) fn web_view_event(&self, event: WebViewEvent) {
        self.host.push_web_view_event(event);
    }

    pub(crate) fn child_view_change(&self, child: ChildId, change: ViewChange) {
        self.host.push_child_view_change(child, change);
    }

    pub(crate) fn set_child_statuses(&self, statuses: Vec<ChildStatus>) {
        self.host.set_child_statuses(statuses);
    }

    fn children(&mut self) -> Vec<ChildPlacements> {
        let instance = self.instance;
        let generation = self.generation;
        let mut messages = Vec::new();
        for (region, state) in &mut self.regions {
            let current = bounded(&state.children, &state.occluders);
            if state.reported_children.as_ref() == Some(&current) {
                continue;
            }
            state.reported_children = Some(current.clone());
            let (children, occluders) = current;
            messages.push(ChildPlacements {
                instance,
                region: *region,
                generation,
                children,
                occluders,
            });
        }
        messages
    }

    fn retain_child_statuses(&self) {
        let live: Vec<ChildId> = self
            .regions
            .values()
            .flat_map(|state| state.children.iter().map(|child| child.child))
            .collect();
        self.host.retain_child_statuses(&live);
    }

    fn scale_factor(&self, region: EditorRegion) -> f32 {
        self.placement(region)
            .map_or(1.0, |placement| placement.scale_factor())
    }

    fn placement(&self, region: EditorRegion) -> Option<&ScreenPlacement> {
        self.regions
            .get(&region)
            .and_then(|state| state.placement.as_ref())
    }

    fn rect(&self, region: EditorRegion) -> beui::Rect {
        let state = self.regions.get(&region);
        let (Some(placement), Some(metrics)) = (
            state.and_then(|state| state.placement.as_ref()),
            state.and_then(|state| state.metrics.as_ref()),
        ) else {
            return beui::Rect::ZERO;
        };
        let scale = placement.scale_factor();
        beui::Rect::from_min_size(
            beui::pos2(
                placement.x as f32 / scale - metrics.visible_x,
                placement.y as f32 / scale - metrics.visible_y,
            ),
            beui::vec2(metrics.logical_width, metrics.logical_height),
        )
    }

    pub(crate) fn run(&mut self, region: EditorRegion, generation: u64) -> beui::FrameOutput {
        self.generation = generation;
        let scale_factor = self.scale_factor(region);
        let host = self.rect(region);
        let spec = self
            .regions
            .get(&region)
            .and_then(|state| state.frame.clone())
            .unwrap_or_default();
        let creating = self.creating;
        let mut draft = self
            .artifact
            .as_ref()
            .filter(|_| region == EditorRegion::ArtifactSettings)
            .map(|artifact| artifact.draft.clone());
        let (ratio, pixels_per_point) = {
            let state = self.beui.entry(region).or_insert_with(BeuiRegion::new);
            state.context.set_pixels_per_point(scale_factor);
            let pixels_per_point = state.context.pixels_per_point();
            (scale_factor / pixels_per_point, pixels_per_point)
        };
        self.host.begin_region(region, host.min.to_vec2());
        self.host
            .begin_beui_frame(ratio, pixels_per_point, spec.chrome == FrameChrome::Drawn);
        let drag = self.drag.and_then(|(dragged, drag)| {
            (dragged == region).then(|| BlockDrag {
                position: drag.position + host.min.to_vec2(),
                ..drag
            })
        });
        self.host.set_drag(drag);
        let files = self.files.as_ref().and_then(|(dropped, files)| {
            (*dropped == region).then(|| crate::host::FileDrop {
                position: files.position + host.min.to_vec2(),
                ..files.clone()
            })
        });
        let delivered_drop = drag.is_some_and(|drag| drag.dropped);
        let delivered_files = files.as_ref().is_some_and(|files| files.dropped);
        self.host.set_files(files);
        let app = &mut self.app;
        let state = self.beui.entry(region).or_insert_with(BeuiRegion::new);
        let events = std::mem::take(&mut state.events);
        let context = state.context.clone();
        let frame = scaled(host, ratio);
        let drawn = spec.chrome == FrameChrome::Drawn;
        if region == EditorRegion::Frame && !creating && state.chrome.is_none() {
            let editor = app
                .editor()
                .expect("connect is called before the view is built");
            state.chrome = Some(BeuiFrame::build(&editor, || app.view()));
        }
        let mut exit = false;
        let mut content_rect = None;
        let mut painted = Vec::new();
        let mut floating = Vec::new();
        let output = context.run(beui::RawInput { events }, |context| match region {
            EditorRegion::Frame if creating => app.creation(context, frame),
            EditorRegion::Frame => {
                let chrome = state
                    .chrome
                    .as_mut()
                    .expect("the frame chrome was just built");
                let set_bar = chrome.set_bar();
                let bar = FrameBar {
                    shown: drawn && (spec.top_bar || spec.content.is_some()),
                    closable: spec.content.is_some(),
                };
                beui::reactive::with_reactive_scope(chrome.document_mut(), || {
                    set_bar.set(bar);
                    app.update();
                });
                chrome.document_mut().show(context, frame);
                app.after_layout(chrome.document());
                content_rect = chrome.document().node_rect(chrome.content());
                exit = chrome.exit().get() || beui_frame::escaped(context);
                painted = vec![frame];
                floating = chrome.document().overlay_rects();
            }
            EditorRegion::Preview => app.preview(context, frame),
            EditorRegion::ArtifactSettings => {
                if let Some(draft) = draft.as_mut() {
                    app.artifact_settings(context, frame, draft);
                }
            }
        });

        let mut output = output;
        if let Some(delay) = self.host.take_frame_request() {
            output.repaint_after = output.repaint_after.min(delay);
        }
        if let (Some(artifact), Some(draft)) = (self.artifact.as_mut(), draft) {
            artifact.edited |= artifact.draft != draft;
            artifact.draft = draft;
        }
        let (placed, occluders) = self.host.end_region(region);
        self.host.set_drag(None);
        self.host.set_files(None);
        if delivered_drop {
            self.drag = None;
        }
        if delivered_files {
            self.files = None;
        }
        let origin = host.min.to_vec2();
        let screen = self.placement(region).map(|placement| placement.screen);
        let content = content_rect.unwrap_or(frame);
        self.leaving |= exit;
        self.used(region, host);
        let reported = |rect: beui::Rect| plugin_rect(scaled(rect, ratio.recip()), origin);
        let reported_content = self
            .host
            .take_beui_content()
            .map(|reported| reported.intersect(content))
            .filter(|reported| reported.is_positive())
            .unwrap_or(content);
        if let (Some(state), Some(screen)) = (self.regions.get_mut(&region), screen) {
            state.children = placed;
            state.occluders = occluders;
            state.cursor = match context.touch_emulation() {
                true => CursorIcon::Crosshair,
                false => beui_cursor(output.cursor_icon),
            };
            state.ime = output.ime.map(|area| ImeArea {
                rect: reported(area.rect),
                cursor: reported(area.cursor),
            });
            state.report = (region == EditorRegion::Frame).then(|| FrameReport {
                screen,
                content: reported(reported_content),
                painted: painted.iter().map(|rect| reported(*rect)).collect(),
                floating: floating.iter().map(|rect| reported(*rect)).collect(),
            });
        }
        self.host.grab_cursor(self.pointer_locked());
        if let Some(text) = &output.copied_text {
            self.copied.push(text.clone());
        }
        self.paste_requested |= output.paste_requested;
        output
    }

    fn pointer_locked(&self) -> bool {
        self.beui
            .values()
            .any(|region| region.context.pointer_locked())
    }

    pub(crate) fn input(&mut self, region: EditorRegion, event: &InputEvent) {
        let scale_factor = self.scale_factor(region);
        let origin = self.rect(region).min.to_vec2();
        let state = self.beui.entry(region).or_insert_with(BeuiRegion::new);
        let ratio = state
            .context
            .simulated_pixels_per_point()
            .map_or(1.0, |simulated| scale_factor / simulated);
        let at = |x: f32, y: f32| beui::pos2((x + origin.x) * ratio, (y + origin.y) * ratio);
        let emulating = state.context.touch_emulation();
        if !emulating && state.emulated_touch {
            state.emulated_touch = false;
            state.emulate_touch(beui::TouchPhase::Cancel);
        }
        match event {
            InputEvent::PointerMoved { x, y } => {
                state.pointer = at(*x, *y);
                if !emulating {
                    state.events.push(beui::Event::PointerMoved(state.pointer));
                } else if state.emulated_touch {
                    state.emulate_touch(beui::TouchPhase::Move);
                }
            }
            InputEvent::PointerLeft if !emulating => state.events.push(beui::Event::PointerGone),
            InputEvent::PointerLeft => {}
            InputEvent::PointerButton {
                button: PointerButton::Primary,
                pressed,
                x,
                y,
            } if emulating => {
                state.pointer = at(*x, *y);
                if *pressed != state.emulated_touch {
                    state.emulated_touch = *pressed;
                    state.emulate_touch(if *pressed {
                        beui::TouchPhase::Start
                    } else {
                        beui::TouchPhase::End
                    });
                }
            }
            InputEvent::PointerButton { .. } if emulating => {}
            InputEvent::PointerButton {
                button,
                pressed,
                x,
                y,
            } => {
                let Some(button) = beui_button(*button) else {
                    return;
                };
                state.pointer = at(*x, *y);
                state.events.push(beui::Event::PointerButton {
                    pos: state.pointer,
                    button,
                    pressed: *pressed,
                    modifiers: state.modifiers,
                });
            }
            InputEvent::Wheel { x, y, unit } => {
                let scale = match unit {
                    WheelUnit::Pixels => 1.0,
                    WheelUnit::Lines => WHEEL_LINE,
                    WheelUnit::Pages => WHEEL_PAGE,
                };
                state
                    .events
                    .push(beui::Event::Scroll(beui::vec2(x * scale, y * scale)));
            }
            InputEvent::Touch {
                device,
                finger,
                phase,
                x,
                y,
                force,
            } => state.events.push(beui::Event::Touch {
                id: beui::TouchId {
                    device: *device,
                    finger: *finger,
                },
                phase: beui_touch_phase(*phase),
                pos: at(*x, *y),
                force: *force,
            }),
            InputEvent::Key {
                key,
                pressed,
                repeat,
            } => {
                let Some(key) = beui_key(*key) else {
                    return;
                };
                state.events.push(beui::Event::Key {
                    key,
                    pressed: *pressed,
                    repeat: *repeat,
                    modifiers: state.modifiers,
                });
            }
            InputEvent::Text(text) | InputEvent::Paste(text) => {
                state.events.push(beui::Event::Text(text.clone()));
            }
            InputEvent::Modifiers(modifiers) => {
                state.modifiers = beui::Modifiers {
                    alt: modifiers.alt,
                    ctrl: modifiers.control || modifiers.command,
                    shift: modifiers.shift,
                };
                state.events.push(beui::Event::Modifiers(state.modifiers));
            }
            InputEvent::Zoom { factor } => {
                state.events.push(beui::Event::Zoom(*factor));
            }
            InputEvent::PointerMotion { x, y } => {
                state
                    .events
                    .push(beui::Event::PointerMotion(beui::vec2(*x, *y) * ratio));
            }
            InputEvent::Focus(false) => {
                state.emulated_touch = false;
                state.events.push(beui::Event::Focus(false));
            }
            InputEvent::Ime(ime) => state.events.push(beui::Event::Ime(match ime {
                ImeInput::Enabled => beui::ImeEvent::Enabled,
                ImeInput::Preedit(text) => beui::ImeEvent::Preedit(text.clone()),
                ImeInput::Commit(text) => beui::ImeEvent::Commit(text.clone()),
                ImeInput::Disabled => beui::ImeEvent::Disabled,
            })),
            InputEvent::Focus(_) => {}
        }
    }
}

fn beui_button(button: PointerButton) -> Option<beui::PointerButton> {
    match button {
        PointerButton::Primary => Some(beui::PointerButton::Primary),
        PointerButton::Secondary => Some(beui::PointerButton::Secondary),
        PointerButton::Middle => Some(beui::PointerButton::Middle),
        PointerButton::Back | PointerButton::Forward | PointerButton::Other(_) => None,
    }
}

fn beui_touch_phase(phase: block_plugin_api::TouchPhase) -> beui::TouchPhase {
    match phase {
        block_plugin_api::TouchPhase::Start => beui::TouchPhase::Start,
        block_plugin_api::TouchPhase::Move => beui::TouchPhase::Move,
        block_plugin_api::TouchPhase::End => beui::TouchPhase::End,
        block_plugin_api::TouchPhase::Cancel => beui::TouchPhase::Cancel,
    }
}

fn beui_key(key: Key) -> Option<beui::Key> {
    let key = match key {
        Key::ArrowDown => beui::Key::ArrowDown,
        Key::ArrowLeft => beui::Key::ArrowLeft,
        Key::ArrowRight => beui::Key::ArrowRight,
        Key::ArrowUp => beui::Key::ArrowUp,
        Key::Backspace => beui::Key::Backspace,
        Key::Delete => beui::Key::Delete,
        Key::End => beui::Key::End,
        Key::Enter => beui::Key::Enter,
        Key::Escape => beui::Key::Escape,
        Key::Home => beui::Key::Home,
        Key::OpenBracket => beui::Key::BracketLeft,
        Key::CloseBracket => beui::Key::BracketRight,
        Key::Minus => beui::Key::Minus,
        Key::PageDown => beui::Key::PageDown,
        Key::PageUp => beui::Key::PageUp,
        Key::Plus | Key::Equals => beui::Key::Plus,
        Key::Space => beui::Key::Space,
        Key::Tab => beui::Key::Tab,
        Key::Num0 => beui::Key::Zero,
        Key::Num1 => beui::Key::One,
        Key::Num2 => beui::Key::Two,
        Key::Num3 => beui::Key::Three,
        Key::Num4 => beui::Key::Four,
        Key::Num5 => beui::Key::Five,
        Key::Num6 => beui::Key::Six,
        Key::Num7 => beui::Key::Seven,
        Key::Num8 => beui::Key::Eight,
        Key::Num9 => beui::Key::Nine,
        Key::Backtick => beui::Key::Backtick,
        Key::A => beui::Key::A,
        Key::B => beui::Key::B,
        Key::C => beui::Key::C,
        Key::D => beui::Key::D,
        Key::E => beui::Key::E,
        Key::F => beui::Key::F,
        Key::G => beui::Key::G,
        Key::H => beui::Key::H,
        Key::I => beui::Key::I,
        Key::J => beui::Key::J,
        Key::K => beui::Key::K,
        Key::L => beui::Key::L,
        Key::M => beui::Key::M,
        Key::N => beui::Key::N,
        Key::O => beui::Key::O,
        Key::P => beui::Key::P,
        Key::Q => beui::Key::Q,
        Key::R => beui::Key::R,
        Key::S => beui::Key::S,
        Key::T => beui::Key::T,
        Key::U => beui::Key::U,
        Key::V => beui::Key::V,
        Key::W => beui::Key::W,
        Key::X => beui::Key::X,
        Key::Y => beui::Key::Y,
        Key::Z => beui::Key::Z,
        _ => return None,
    };
    Some(key)
}

fn beui_cursor(cursor: beui::CursorIcon) -> CursorIcon {
    match cursor {
        beui::CursorIcon::Default => CursorIcon::Default,
        beui::CursorIcon::Crosshair => CursorIcon::Crosshair,
        beui::CursorIcon::Grab => CursorIcon::Grab,
        beui::CursorIcon::Grabbing => CursorIcon::Grabbing,
        beui::CursorIcon::NotAllowed => CursorIcon::NotAllowed,
        beui::CursorIcon::PointingHand => CursorIcon::Pointer,
        beui::CursorIcon::ResizeHorizontal => CursorIcon::ResizeHorizontal,
        beui::CursorIcon::ResizeVertical => CursorIcon::ResizeVertical,
        beui::CursorIcon::ResizeNeSw => CursorIcon::ResizeNeSw,
        beui::CursorIcon::ResizeNwSe => CursorIcon::ResizeNwSe,
        beui::CursorIcon::Text => CursorIcon::Text,
        beui::CursorIcon::Wait => CursorIcon::Wait,
        beui::CursorIcon::None => CursorIcon::None,
        beui::CursorIcon::Move => CursorIcon::Move,
        beui::CursorIcon::Progress => CursorIcon::Progress,
        beui::CursorIcon::Help => CursorIcon::Help,
        beui::CursorIcon::Alias => CursorIcon::Pointer,
    }
}

fn scaled(rect: beui::Rect, ratio: f32) -> beui::Rect {
    beui::Rect::from_min_max(
        beui::pos2(rect.min.x * ratio, rect.min.y * ratio),
        beui::pos2(rect.max.x * ratio, rect.max.y * ratio),
    )
}

fn plugin_rect(rect: beui::Rect, origin: beui::Vec2) -> ChildRect {
    let rect = rect.translate(-origin);
    ChildRect {
        x: rect.min.x,
        y: rect.min.y,
        width: rect.width(),
        height: rect.height(),
    }
}

fn bounded(
    children: &[ChildPlacement],
    occluders: &[Occluder],
) -> (Vec<ChildPlacement>, Vec<Occluder>) {
    let kept = children.len().min(MAX_CHILDREN);
    (
        children[..kept].to_vec(),
        occluders
            .iter()
            .take(MAX_COLLECTION_ITEMS)
            .map(|occluder| Occluder {
                after: occluder.after.min(kept as u32),
                rect: occluder.rect,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
