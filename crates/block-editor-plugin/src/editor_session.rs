use block_client::{
    BlockClient,
    presence::{UserActive, pick_free_color},
};
use block_plugin_api::{
    ArtifactDescription, BlockPick, ChildId, ChildPlacement, ChildPlacements, ChildRect,
    ChildStatus, CreationOutcome, CursorIcon, EditorBand, EditorInstanceId, EditorMessage,
    EditorRegion, FetchResult, FilePick, FrameChrome, FrameReport, FrameSpec, ImeArea, ImeInput,
    InputEvent, MAX_CHILDREN, MAX_COLLECTION_ITEMS, Message, Occluder, PointerButton, RegionSize,
    ScreenPlacement, ScreenRequest, ViewChange, ViewportMetrics, WebViewEvent, WheelUnit,
};
use block_ui::BlockCatalog;
use eframe::egui;
use std::{collections::HashMap, marker::PhantomData, rc::Rc, sync::Arc};
use uuid::Uuid;

use crate::{EditorHost, Waker, beui_frame, beui_frame::BeuiFrame, host::BlockDrag};

const WHEEL_LINE: f32 = 40.0;
const WHEEL_PAGE: f32 = 400.0;

pub(crate) struct EditorSession {
    app: Box<dyn AppUi>,
    beui: Option<HashMap<EditorRegion, BeuiRegion>>,
    chrome: Rc<Vec<EditorBand>>,
    instance: EditorInstanceId,
    regions: HashMap<EditorRegion, RegionState>,
    host: EditorHost,
    block: Option<(Arc<BlockClient>, Uuid)>,
    drag: Option<(EditorRegion, BlockDrag)>,
    files: Option<(EditorRegion, crate::host::FileDrop)>,
    intrinsic: Option<egui::Vec2>,
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
    input: egui::RawInput,
    placement: Option<ScreenPlacement>,
    metrics: Option<ViewportMetrics>,
    frame: Option<FrameSpec>,
    used: Option<egui::Vec2>,
    reported: Option<egui::Vec2>,
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
    fn beui_view(&mut self) -> beui::NodeId {
        unreachable!("beui_view is only called on a BeuiApp instance")
    }
    fn beui_update(&mut self) {}
    fn beui_after_layout(&mut self, _document: &beui::Document) {}
    fn beui_creation(&mut self, _context: &beui::Context, _rect: beui::Rect) {}
    fn beui_preview(&mut self, _context: &beui::Context, _rect: beui::Rect) {}
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid);
    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>);
    fn create_block(&mut self) -> Result<Uuid, String>;
    fn creation_ui(&mut self, ui: &mut egui::Ui);
    fn connect_artifact(
        &mut self,
        host: EditorHost,
        client: Arc<BlockClient>,
        artifact: crate::Artifact,
    );
    fn describe_artifact(&mut self, data: &[u8]) -> ArtifactDescription;
    fn artifact_settings_ui(&mut self, ui: &mut egui::Ui, data: &mut Vec<u8>);
    fn regenerate_artifact(&mut self, data: &[u8]);
    fn poll_artifact(&mut self) -> Option<Result<(), String>>;
    fn main_ui(&mut self, ui: &mut egui::Ui);
    fn toolbar_ui(&mut self, ui: &mut egui::Ui);
    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui);
    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui);
    fn preview_ui(&mut self, ui: &mut egui::Ui);
    fn intrinsic_size(&mut self) -> Option<egui::Vec2>;
    fn set_intrinsic_size(&mut self, size: egui::Vec2);
    fn aspect_ratio(&mut self) -> Option<f32>;
    fn presence_visible(&mut self, visible: bool);
    fn reveal_presence(&mut self, client_id: u64);
    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool;
}

impl<A: crate::App> AppUi for A {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        crate::App::connect(self, host, client, block_id);
    }

    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>) {
        crate::App::connect_creation(self, host, client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        crate::App::create_block(self)
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::creation_ui(self, ui);
    }

    fn connect_artifact(
        &mut self,
        host: EditorHost,
        client: Arc<BlockClient>,
        artifact: crate::Artifact,
    ) {
        crate::App::connect_artifact(self, host, client, artifact);
    }

    fn describe_artifact(&mut self, data: &[u8]) -> ArtifactDescription {
        match crate::App::describe_artifact(self, data) {
            Ok(description) => ArtifactDescription::Described {
                source: description.source.into_bytes(),
                summary: description.summary,
            },
            Err(error) => ArtifactDescription::Unreadable(error),
        }
    }

    fn artifact_settings_ui(&mut self, ui: &mut egui::Ui, data: &mut Vec<u8>) {
        crate::App::artifact_settings_ui(self, ui, data);
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        crate::App::regenerate_artifact(self, data);
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        crate::App::poll_artifact(self)
    }

    fn main_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::ui(self, ui);
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::toolbar_ui(self, ui);
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::left_sidebar_ui(self, ui);
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::right_sidebar_ui(self, ui);
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        crate::App::preview_ui(self, ui);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        crate::App::intrinsic_size(self)
    }

    fn set_intrinsic_size(&mut self, size: egui::Vec2) {
        crate::App::set_intrinsic_size(self, size);
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        crate::App::aspect_ratio(self)
    }

    fn presence_visible(&mut self, visible: bool) {
        crate::App::presence_visible(self, visible);
    }

    fn reveal_presence(&mut self, client_id: u64) {
        crate::App::reveal_presence(self, client_id);
    }

    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        crate::App::replace_child(self, old, new)
    }
}

struct BeuiHolder<A: crate::BeuiApp> {
    editor: Option<crate::Editor>,
    preview: Option<crate::Editor>,
    preview_document: Option<beui::Document>,
    creation: Option<crate::Creation>,
    dialog: Option<beui::Document>,
    artifacts: Option<crate::Artifacts>,
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
            app: PhantomData,
        }
    }
}

impl<A: crate::BeuiApp> AppUi for BeuiHolder<A> {
    fn beui_view(&mut self) -> beui::NodeId {
        let editor = self
            .editor
            .clone()
            .expect("connect is called before the view is built");
        A::view(editor)
    }

    fn beui_update(&mut self) {
        if let Some(editor) = &self.editor {
            editor.begin_frame();
        }
    }

    fn beui_after_layout(&mut self, document: &beui::Document) {
        if let Some(editor) = &self.editor {
            editor.end_frame(document);
        }
    }

    fn beui_preview(&mut self, context: &beui::Context, rect: beui::Rect) {
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

    fn beui_creation(&mut self, context: &beui::Context, rect: beui::Rect) {
        let (Some(creation), Some(dialog)) = (self.creation.as_ref(), self.dialog.as_mut()) else {
            return;
        };
        let creation = creation.clone();
        beui::reactive::with_reactive_scope(dialog, move || creation.begin_frame());
        dialog.show(context, rect);
    }

    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.editor = Some(crate::Editor::new(
            host.clone(),
            Arc::clone(&client),
            block_id,
        ));
        self.preview = Some(crate::Editor::new(host, client, block_id));
        self.preview_document = None;
    }

    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>) {
        let creation = crate::Creation::new(host, client);
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

    fn creation_ui(&mut self, _ui: &mut egui::Ui) {}

    fn connect_artifact(
        &mut self,
        host: EditorHost,
        client: Arc<BlockClient>,
        artifact: crate::Artifact,
    ) {
        let artifacts = crate::Artifacts::new(host, client, artifact);
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

    fn artifact_settings_ui(&mut self, ui: &mut egui::Ui, data: &mut Vec<u8>) {
        A::artifact_settings_ui(ui, data);
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        if let Some(artifacts) = &self.artifacts {
            artifacts.regenerate(data);
        }
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        self.artifacts.as_ref()?.poll()
    }

    fn main_ui(&mut self, _ui: &mut egui::Ui) {}

    fn toolbar_ui(&mut self, _ui: &mut egui::Ui) {}

    fn left_sidebar_ui(&mut self, _ui: &mut egui::Ui) {}

    fn right_sidebar_ui(&mut self, _ui: &mut egui::Ui) {}

    fn preview_ui(&mut self, _ui: &mut egui::Ui) {}

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        self.editor
            .as_ref()
            .and_then(crate::Editor::intrinsic_size)
            .or_else(A::intrinsic_size)
            .map(|size| egui::vec2(size.x, size.y))
    }

    fn set_intrinsic_size(&mut self, size: egui::Vec2) {
        if let Some(editor) = &self.editor {
            editor.report_resize(beui::Vec2::new(size.x, size.y));
        }
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        A::aspect_ratio()
    }

    fn presence_visible(&mut self, _visible: bool) {}

    fn reveal_presence(&mut self, _client_id: u64) {}

    fn replace_child(&mut self, _old: Uuid, _new: Uuid) -> bool {
        false
    }
}

impl EditorSession {
    pub(crate) fn new<A: crate::App>(
        chrome: Rc<Vec<EditorBand>>,
        instance: EditorInstanceId,
        waker: Waker,
    ) -> Self {
        Self::of(Box::new(A::default()), None, chrome, instance, waker)
    }

    pub(crate) fn beui<A: crate::BeuiApp>(
        chrome: Rc<Vec<EditorBand>>,
        instance: EditorInstanceId,
        waker: Waker,
    ) -> Self {
        Self::of(
            Box::new(BeuiHolder::<A>::new()),
            Some(HashMap::new()),
            chrome,
            instance,
            waker,
        )
    }

    fn of(
        app: Box<dyn AppUi>,
        beui: Option<HashMap<EditorRegion, BeuiRegion>>,
        chrome: Rc<Vec<EditorBand>>,
        instance: EditorInstanceId,
        waker: Waker,
    ) -> Self {
        Self {
            app,
            beui,
            chrome,
            instance,
            regions: HashMap::new(),
            host: EditorHost::new(waker),
            block: None,
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

    pub(crate) fn set_pasted_image(&self, request: u64, image: block_plugin_api::ClipboardImage) {
        self.host.set_pasted_image(request, image);
    }

    pub(crate) fn set_audio(&self, status: block_plugin_api::AudioStatus) {
        self.host.set_audio(status);
    }

    pub(crate) fn set_client_id(&self, client_id: Uuid) {
        self.host.set_client_id(client_id);
    }

    pub(crate) fn set_editable(&self, editable: bool) {
        self.host.set_editable(editable);
    }

    pub(crate) fn set_focused_block(&self, focused: crate::host::FocusedBlock) {
        self.host.set_focused_block(focused);
    }

    pub(crate) fn show_block(
        &self,
        block_id: Uuid,
        block_type: Uuid,
        via: Option<Uuid>,
        from: Option<Uuid>,
    ) {
        self.host.show_block(block_id, block_type, via, from);
    }

    pub(crate) fn set_artifacts(&self, states: Vec<crate::host::ArtifactState>) {
        self.host.set_artifacts(states);
    }

    pub(crate) fn set_view(&self, view: egui::Rect, scale: f32) {
        self.host.set_view(view, scale);
    }

    pub(crate) fn resized(&mut self, size: egui::Vec2) {
        self.app.set_intrinsic_size(size);
    }

    pub(crate) fn presence_visible(&mut self, visible: bool) {
        if let Some((client, block_id)) = &self.block {
            match visible {
                true => {
                    let used = client
                        .presence::<UserActive>(*block_id)
                        .into_iter()
                        .map(|(_, user)| user.color);
                    let color = pick_free_color(used);
                    client.set_presence(*block_id, Some(&UserActive { color }));
                }
                false => client.set_presence::<UserActive>(*block_id, None),
            }
        }
        self.app.presence_visible(visible);
    }

    fn viewers(&self) -> Vec<block_ui::frame::Viewer> {
        let Some((client, block_id)) = &self.block else {
            return Vec::new();
        };
        let mut viewers: Vec<_> = client
            .presence::<UserActive>(*block_id)
            .into_iter()
            .map(|(client_id, user)| block_ui::frame::Viewer {
                client_id,
                color: user.color,
            })
            .collect();
        viewers.sort_by_key(|viewer| viewer.client_id);
        viewers
    }

    pub(crate) fn replace_child(&mut self, request_id: u64, old: Uuid, new: Uuid) {
        let replaced = self.app.replace_child(old, new);
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

    pub(crate) fn connect(&mut self, client: Arc<BlockClient>, block_id: Uuid, block_type: Uuid) {
        if block_client::blocks::watch(&client, block_id, block_type) {
            self.block = Some((Arc::clone(&client), block_id));
        }
        self.app.connect(self.host.clone(), client, block_id);
    }

    pub(crate) fn connect_creation(&mut self, client: Arc<BlockClient>) {
        self.creating = true;
        self.host.set_editable(true);
        self.app.connect_creation(self.host.clone(), client);
    }

    pub(crate) fn connect_artifact(
        &mut self,
        client: Arc<BlockClient>,
        block_id: Uuid,
        block_type: Uuid,
        data: Vec<u8>,
    ) {
        self.artifact = Some(ArtifactState::new(data));
        self.host.set_editable(true);
        self.app.connect_artifact(
            self.host.clone(),
            client,
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
        if let Some(accepted) = self.host.take_drag_accepted() {
            messages.push(Message::Editor(EditorMessage::DragAccepted {
                instance,
                accepted,
            }));
        }
        let intrinsic = self.app.intrinsic_size();
        if intrinsic != self.intrinsic {
            self.intrinsic = intrinsic;
            if let Some(size) = intrinsic {
                messages.push(Message::Editor(EditorMessage::IntrinsicSize {
                    instance,
                    width: size.x,
                    height: size.y,
                }));
            }
        }
        let aspect_ratio = self.app.aspect_ratio();
        if aspect_ratio != self.aspect_ratio {
            self.aspect_ratio = aspect_ratio;
            if let Some(ratio) = aspect_ratio {
                messages.push(Message::Editor(EditorMessage::AspectRatio {
                    instance,
                    ratio,
                }));
            }
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
            if artifact.regenerating {
                if let Some(result) = self.app.poll_artifact() {
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
        }
        if std::mem::take(&mut self.leaving) {
            messages.push(Message::Editor(EditorMessage::LeaveFrame { instance }));
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
        for (request_id, filter) in self.host.take_picks() {
            messages.push(Message::Editor(EditorMessage::PickFile {
                instance,
                request_id,
                filter,
            }));
        }
        for (request_id, filter) in self.host.take_block_picks() {
            messages.push(Message::Editor(EditorMessage::PickBlock {
                instance,
                request_id,
                filter,
            }));
        }
        for request_id in self.host.take_pastes() {
            messages.push(Message::Editor(EditorMessage::PasteImage {
                instance,
                request_id,
            }));
        }
        for (block_id, command) in self.host.take_audio_commands() {
            messages.push(Message::Editor(EditorMessage::PlayAudio {
                instance,
                block_id: block_id.into_bytes(),
                command,
            }));
        }
        for (request_id, url) in self.host.take_fetches() {
            messages.push(Message::Editor(EditorMessage::Fetch {
                instance,
                request_id,
                url,
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

    fn used(&mut self, region: EditorRegion, content: egui::Rect) {
        let origin = self.rect(region).min;
        let Some(state) = self.regions.get_mut(&region) else {
            return;
        };
        let used = (content.max - origin).max(egui::Vec2::ZERO);
        state.used = Some(egui::vec2(used.x.round(), used.y.round()));
    }

    pub(crate) fn file_picked(&self, request_id: u64, pick: FilePick) {
        self.host.set_pick(request_id, pick);
    }

    pub(crate) fn block_picked(&self, request_id: u64, pick: BlockPick) {
        self.host.set_block_pick(request_id, pick);
    }

    pub(crate) fn fetched(&self, request_id: u64, result: FetchResult) {
        self.host.set_fetched(request_id, result);
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

    pub(crate) fn run(
        &mut self,
        region: EditorRegion,
        context: &egui::Context,
        time: f64,
        generation: u64,
    ) -> egui::FullOutput {
        self.generation = generation;
        let scale_factor = self.scale_factor(region);
        let rect = self.rect(region);
        let visible_rect = self.visible_rect(region);
        let state = self.regions.entry(region).or_default();
        state.input.screen_rect = Some(rect);
        state.input.time = Some(time);
        let mut input = std::mem::take(&mut state.input);
        input.viewport_id = viewport_id(region);
        input
            .viewports
            .entry(input.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(scale_factor);
        state.input.focused = input.focused;
        state.input.modifiers = input.modifiers;
        let drag = self.drag.and_then(|(dragged, drag)| {
            (dragged == region).then(|| BlockDrag {
                position: drag.position + rect.min.to_vec2(),
                ..drag
            })
        });
        self.host.set_drag(drag);
        let files = self.files.as_ref().and_then(|(dropped, files)| {
            (*dropped == region).then(|| crate::host::FileDrop {
                position: files.position + rect.min.to_vec2(),
                ..files.clone()
            })
        });
        let delivered_files = files.as_ref().is_some_and(|files| files.dropped);
        self.host.set_files(files);
        let delivered_drop = drag.is_some_and(|drag| drag.dropped);
        let viewers = self.viewers();
        let app = &mut self.app;
        let plain =
            egui::Frame::central_panel(&context.global_style()).inner_margin(egui::Margin::ZERO);
        let creating = self.creating;
        let artifact = &mut self.artifact;
        let mut draft = artifact
            .as_mut()
            .filter(|_| region == EditorRegion::ArtifactSettings)
            .map(|artifact| artifact.draft.clone());
        let spec = self
            .regions
            .get(&region)
            .and_then(|state| state.frame.clone())
            .unwrap_or_default();
        let chrome = Rc::clone(&self.chrome);
        let host = self.host.clone();
        self.host.begin_region(region, rect.min.to_vec2());
        let mut content = rect;
        let mut painted = Vec::new();
        let mut leaving = false;
        let mut reveal = None;
        let mut content_band = rect;
        let output = context.run_ui(input, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(visible_rect));
            match (&mut draft, creating, region) {
                (Some(draft), _, _) => {
                    egui::CentralPanel::default()
                        .frame(plain)
                        .show_inside(ui, |ui| app.artifact_settings_ui(ui, draft));
                }
                (None, true, _) => {
                    egui::CentralPanel::default()
                        .frame(plain)
                        .show_inside(ui, |ui| app.creation_ui(ui));
                }
                (None, false, EditorRegion::Frame) => {
                    let band = |band| chrome.contains(&band) && host.band_shown(band);
                    let mut bands = AppBands {
                        app,
                        background: plain.fill,
                    };
                    let outcome = block_ui::frame::Frame::new(egui::Id::new("plugin frame"))
                        .chrome(match spec.chrome {
                            FrameChrome::Drawn => block_ui::frame::Chrome::Drawn,
                            FrameChrome::None => block_ui::frame::Chrome::None,
                        })
                        .toolbar(band(EditorBand::Toolbar))
                        .left_sidebar(band(EditorBand::LeftSidebar))
                        .right_sidebar(band(EditorBand::RightSidebar))
                        .content(
                            spec.content
                                .map(|content| host_rect(content, rect.min.to_vec2())),
                        )
                        .trail(spec.trail.clone())
                        .viewers(viewers.clone())
                        .show(ui, &mut bands);
                    leaving |= outcome.exit;
                    reveal = outcome.reveal;
                    content_band = outcome.rects.content;
                    painted = outcome.rects.painted().collect();
                }
                (None, false, EditorRegion::Preview) => {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show_inside(ui, |ui| app.preview_ui(ui));
                }
                (None, false, EditorRegion::ArtifactSettings) => {}
            }
            content = ui.min_rect();
        });
        if let (Some(artifact), Some(draft)) = (artifact.as_mut(), draft) {
            artifact.edited |= artifact.draft != draft;
            artifact.draft = draft;
        }
        let floating = floating_rects(context, visible_rect);
        for occluder in &floating {
            self.host.occlude(*occluder);
        }
        let (children, occluders) = self.host.end_region(region);
        let origin = rect.min.to_vec2();
        let screen = self.placement(region).map(|placement| placement.screen);
        if let (Some(state), Some(screen)) = (self.regions.get_mut(&region), screen) {
            state.children = children;
            state.occluders = occluders;
            state.report = (region == EditorRegion::Frame).then(|| FrameReport {
                screen,
                content: plugin_rect(content_band, origin),
                painted: painted
                    .iter()
                    .map(|rect| plugin_rect(*rect, origin))
                    .collect(),
                floating: floating
                    .iter()
                    .map(|rect| plugin_rect(*rect, origin))
                    .collect(),
            });
        }
        self.host.set_drag(None);
        self.host.set_files(None);
        if delivered_drop {
            self.drag = None;
        }
        if delivered_files {
            self.files = None;
        }
        self.leaving |= leaving;
        if let Some(client_id) = reveal {
            self.app.reveal_presence(client_id);
        }
        self.used(region, content);
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                if !text.is_empty() {
                    self.copied.push(text.clone());
                }
            }
        }
        let cursor = cursor_icon(output.platform_output.cursor_icon);
        let ime = output.platform_output.ime.map(|ime| ImeArea {
            rect: plugin_rect(ime.rect, origin),
            cursor: plugin_rect(ime.cursor_rect, origin),
        });
        if let Some(state) = self.regions.get_mut(&region) {
            state.cursor = cursor;
            state.ime = ime;
        }
        output
    }

    pub(crate) fn scale_factor(&self, region: EditorRegion) -> f32 {
        self.placement(region)
            .map_or(1.0, |placement| placement.scale_factor())
    }

    fn placement(&self, region: EditorRegion) -> Option<&ScreenPlacement> {
        self.regions
            .get(&region)
            .and_then(|state| state.placement.as_ref())
    }

    pub(crate) fn visible_rect(&self, region: EditorRegion) -> egui::Rect {
        let Some(placement) = self.placement(region) else {
            return egui::Rect::ZERO;
        };
        let scale = placement.scale_factor();
        egui::Rect::from_min_size(
            egui::pos2(placement.x as f32 / scale, placement.y as f32 / scale),
            egui::vec2(
                placement.width as f32 / scale,
                placement.height as f32 / scale,
            ),
        )
    }

    fn rect(&self, region: EditorRegion) -> egui::Rect {
        let state = self.regions.get(&region);
        let (Some(placement), Some(metrics)) = (
            state.and_then(|state| state.placement.as_ref()),
            state.and_then(|state| state.metrics.as_ref()),
        ) else {
            return egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO);
        };
        let scale = placement.scale_factor();
        egui::Rect::from_min_size(
            egui::pos2(
                placement.x as f32 / scale - metrics.visible_x,
                placement.y as f32 / scale - metrics.visible_y,
            ),
            egui::vec2(metrics.logical_width, metrics.logical_height),
        )
    }

    pub(crate) fn is_beui(&self) -> bool {
        self.beui.is_some()
    }

    pub(crate) fn run_beui(
        &mut self,
        region: EditorRegion,
        generation: u64,
    ) -> Option<beui::FrameOutput> {
        self.generation = generation;
        let scale_factor = self.scale_factor(region);
        let host = self.rect(region);
        let spec = self
            .regions
            .get(&region)
            .and_then(|state| state.frame.clone())
            .unwrap_or_default();
        let creating = self.creating;
        let (ratio, pixels_per_point) = {
            let beui = self.beui.as_mut()?;
            let state = beui.entry(region).or_insert_with(BeuiRegion::new);
            state.context.set_pixels_per_point(scale_factor);
            let pixels_per_point = state.context.pixels_per_point();
            (scale_factor / pixels_per_point, pixels_per_point)
        };
        self.host.begin_region(region, host.min.to_vec2());
        self.host
            .begin_beui_frame(ratio, pixels_per_point, spec.chrome == FrameChrome::Drawn);
        let app = &mut self.app;
        let beui = self.beui.as_mut()?;
        let state = beui.entry(region).or_insert_with(BeuiRegion::new);
        let events = std::mem::take(&mut state.events);
        let context = state.context.clone();
        let rect = scaled(host, ratio);

        let frame = beui::Rect::from_min_max(
            beui::pos2(rect.min.x, rect.min.y),
            beui::pos2(rect.max.x, rect.max.y),
        );
        let drawn = spec.chrome == FrameChrome::Drawn;
        if region == EditorRegion::Frame && !creating && state.chrome.is_none() {
            state.chrome = Some(BeuiFrame::build(|| app.beui_view()));
        }
        let mut exit = false;
        let mut content_rect = None;
        let mut painted = Vec::new();
        let output = context.run(beui::RawInput { events }, |context| match region {
            EditorRegion::Frame if creating => app.beui_creation(context, frame),
            EditorRegion::Frame => {
                let chrome = state
                    .chrome
                    .as_mut()
                    .expect("the frame chrome was just built");
                let set_trail = chrome.set_trail();
                let set_shown = chrome.set_shown();
                let trail = spec.trail.clone();
                beui::reactive::with_reactive_scope(chrome.document_mut(), || {
                    set_trail.set(trail);
                    set_shown.set(drawn);
                    app.beui_update();
                });
                chrome.document_mut().show(context, frame);
                app.beui_after_layout(chrome.document());
                content_rect = chrome.document().node_rect(chrome.content());
                exit = chrome.exit().get() || beui_frame::escaped(context);
                painted = vec![frame];
            }
            EditorRegion::Preview => app.beui_preview(context, frame),
            EditorRegion::ArtifactSettings => {}
        });

        let (placed, occluders) = self.host.end_region(region);
        let origin = host.min.to_vec2();
        let screen = self.placement(region).map(|placement| placement.screen);
        let content = content_rect.unwrap_or(frame);
        self.leaving |= exit;
        self.used(region, host);
        let reported =
            |rect: beui::Rect| plugin_rect(scaled(egui_rect(rect), ratio.recip()), origin);
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
            state.report = (region == EditorRegion::Frame).then(|| FrameReport {
                screen,
                content: reported(reported_content),
                painted: painted.iter().map(|rect| reported(*rect)).collect(),
                floating: Vec::new(),
            });
        }
        if let Some(text) = &output.copied_text {
            self.copied.push(text.clone());
        }
        self.paste_requested |= output.paste_requested;
        Some(output)
    }

    fn beui_input(&mut self, region: EditorRegion, event: &InputEvent) {
        let scale_factor = self.scale_factor(region);
        let origin = self.rect(region).min.to_vec2();
        let Some(beui) = self.beui.as_mut() else {
            return;
        };
        let state = beui.entry(region).or_insert_with(BeuiRegion::new);
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
                logical,
                pressed,
                repeat,
                ..
            } => {
                let Some(key) = beui_key(logical) else {
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
            InputEvent::Focus(false) => {
                state.emulated_touch = false;
                state.events.push(beui::Event::Focus(false));
            }
            InputEvent::PointerMotion { .. } | InputEvent::Ime(_) | InputEvent::Focus(_) => {}
        }
    }

    pub(crate) fn input(&mut self, region: EditorRegion, event: &InputEvent) {
        if self.beui.is_some() {
            return self.beui_input(region, event);
        }
        let origin = self.rect(region).min.to_vec2();
        let Some(state) = self.regions.get_mut(&region) else {
            return;
        };
        match event {
            InputEvent::PointerMoved { x, y } => state
                .input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(*x, *y) + origin)),
            InputEvent::PointerMotion { x, y } => state
                .input
                .events
                .push(egui::Event::MouseMoved(egui::vec2(*x, *y))),
            InputEvent::PointerButton {
                button,
                pressed,
                x,
                y,
            } => {
                state.input.events.push(egui::Event::PointerButton {
                    pos: egui::pos2(*x, *y) + origin,
                    button: pointer_button(*button),
                    pressed: *pressed,
                    modifiers: state.input.modifiers,
                });
            }
            InputEvent::Wheel { x, y, unit } => {
                state.input.events.push(egui::Event::MouseWheel {
                    unit: wheel_unit(*unit),
                    delta: egui::vec2(*x, *y),
                    phase: egui::TouchPhase::Move,
                    modifiers: state.input.modifiers,
                });
            }
            InputEvent::Touch {
                device,
                finger,
                phase,
                x,
                y,
                force,
            } => state.input.events.push(egui::Event::Touch {
                device_id: egui::TouchDeviceId(*device),
                id: egui::TouchId(*finger),
                phase: egui_touch_phase(*phase),
                pos: egui::pos2(*x, *y) + origin,
                force: *force,
            }),
            InputEvent::Zoom { factor } => state.input.events.push(egui::Event::Zoom(*factor)),
            InputEvent::Key {
                logical,
                pressed,
                repeat,
                ..
            } => {
                if let Some(key) = egui::Key::from_name(logical) {
                    state.input.events.push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: *pressed,
                        repeat: *repeat,
                        modifiers: state.input.modifiers,
                    });
                }
            }
            InputEvent::Text(text) => state.input.events.push(egui::Event::Text(text.clone())),
            InputEvent::Paste(text) => state.input.events.push(egui::Event::Paste(text.clone())),
            InputEvent::Modifiers(modifiers) => {
                state.input.modifiers = egui::Modifiers {
                    alt: modifiers.alt,
                    ctrl: modifiers.control,
                    shift: modifiers.shift,
                    mac_cmd: false,
                    command: modifiers.command,
                };
            }
            InputEvent::Ime(ime) => state.input.events.push(egui::Event::Ime(match ime {
                ImeInput::Enabled => egui::ImeEvent::Enabled,
                ImeInput::Preedit(text) => egui::ImeEvent::Preedit(text.clone()),
                ImeInput::Commit(text) => egui::ImeEvent::Commit(text.clone()),
                ImeInput::Disabled => egui::ImeEvent::Disabled,
            })),
            InputEvent::Focus(focused) => state.input.focused = *focused,
        }
    }
}

struct AppBands<'a> {
    app: &'a mut Box<dyn AppUi>,
    background: egui::Color32,
}

impl block_ui::frame::FrameBands for AppBands<'_> {
    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        self.app.toolbar_ui(ui);
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.app.left_sidebar_ui(ui);
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.app.right_sidebar_ui(ui);
    }

    fn content_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, self.background);
        self.app.main_ui(ui);
    }
}

fn egui_rect(rect: beui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(rect.min.x, rect.min.y),
        egui::pos2(rect.max.x, rect.max.y),
    )
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

fn egui_touch_phase(phase: block_plugin_api::TouchPhase) -> egui::TouchPhase {
    match phase {
        block_plugin_api::TouchPhase::Start => egui::TouchPhase::Start,
        block_plugin_api::TouchPhase::Move => egui::TouchPhase::Move,
        block_plugin_api::TouchPhase::End => egui::TouchPhase::End,
        block_plugin_api::TouchPhase::Cancel => egui::TouchPhase::Cancel,
    }
}

fn beui_key(logical: &str) -> Option<beui::Key> {
    let key = match logical {
        "ArrowDown" => beui::Key::ArrowDown,
        "ArrowLeft" => beui::Key::ArrowLeft,
        "ArrowRight" => beui::Key::ArrowRight,
        "ArrowUp" => beui::Key::ArrowUp,
        "Backspace" => beui::Key::Backspace,
        "Delete" => beui::Key::Delete,
        "End" => beui::Key::End,
        "Enter" => beui::Key::Enter,
        "Escape" => beui::Key::Escape,
        "Home" => beui::Key::Home,
        "-" | "Minus" => beui::Key::Minus,
        "PageDown" => beui::Key::PageDown,
        "PageUp" => beui::Key::PageUp,
        "+" | "=" | "Plus" => beui::Key::Plus,
        "Space" => beui::Key::Space,
        "Tab" => beui::Key::Tab,
        "0" => beui::Key::Zero,
        "A" => beui::Key::A,
        "B" => beui::Key::B,
        "C" => beui::Key::C,
        "D" => beui::Key::D,
        "E" => beui::Key::E,
        "F" => beui::Key::F,
        "G" => beui::Key::G,
        "H" => beui::Key::H,
        "I" => beui::Key::I,
        "J" => beui::Key::J,
        "K" => beui::Key::K,
        "L" => beui::Key::L,
        "M" => beui::Key::M,
        "N" => beui::Key::N,
        "O" => beui::Key::O,
        "P" => beui::Key::P,
        "Q" => beui::Key::Q,
        "R" => beui::Key::R,
        "S" => beui::Key::S,
        "T" => beui::Key::T,
        "U" => beui::Key::U,
        "V" => beui::Key::V,
        "W" => beui::Key::W,
        "X" => beui::Key::X,
        "Y" => beui::Key::Y,
        "Z" => beui::Key::Z,
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
        beui::CursorIcon::Text => CursorIcon::Text,
        beui::CursorIcon::Wait => CursorIcon::Wait,
    }
}

fn scaled(rect: egui::Rect, ratio: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(rect.min.x * ratio, rect.min.y * ratio),
        egui::pos2(rect.max.x * ratio, rect.max.y * ratio),
    )
}

fn host_rect(rect: ChildRect, origin: egui::Vec2) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.x, rect.y) + origin,
        egui::vec2(rect.width, rect.height),
    )
}

fn plugin_rect(rect: egui::Rect, origin: egui::Vec2) -> ChildRect {
    let rect = rect.translate(-origin);
    ChildRect {
        x: rect.min.x,
        y: rect.min.y,
        width: rect.width(),
        height: rect.height(),
    }
}

pub(crate) fn viewport_id(region: EditorRegion) -> egui::ViewportId {
    egui::ViewportId(egui::Id::new(("plugin editor region", region)))
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

fn floating_rects(context: &egui::Context, visible: egui::Rect) -> Vec<egui::Rect> {
    context.memory(|memory| {
        memory
            .layer_ids()
            .filter(|layer| layer.order >= egui::Order::Middle)
            .filter_map(|layer| memory.area_rect(layer.id))
            .filter(|rect| rect.intersects(visible))
            .collect()
    })
}

fn cursor_icon(cursor: egui::CursorIcon) -> CursorIcon {
    match cursor {
        egui::CursorIcon::None => CursorIcon::None,
        egui::CursorIcon::PointingHand => CursorIcon::Pointer,
        egui::CursorIcon::Text | egui::CursorIcon::VerticalText => CursorIcon::Text,
        egui::CursorIcon::Crosshair | egui::CursorIcon::Cell => CursorIcon::Crosshair,
        egui::CursorIcon::Grab => CursorIcon::Grab,
        egui::CursorIcon::Grabbing => CursorIcon::Grabbing,
        egui::CursorIcon::Move | egui::CursorIcon::AllScroll => CursorIcon::Move,
        egui::CursorIcon::NotAllowed | egui::CursorIcon::NoDrop => CursorIcon::NotAllowed,
        egui::CursorIcon::Wait => CursorIcon::Wait,
        egui::CursorIcon::Progress => CursorIcon::Progress,
        egui::CursorIcon::Help => CursorIcon::Help,
        egui::CursorIcon::ResizeHorizontal
        | egui::CursorIcon::ResizeColumn
        | egui::CursorIcon::ResizeEast
        | egui::CursorIcon::ResizeWest => CursorIcon::ResizeHorizontal,
        egui::CursorIcon::ResizeVertical
        | egui::CursorIcon::ResizeRow
        | egui::CursorIcon::ResizeNorth
        | egui::CursorIcon::ResizeSouth => CursorIcon::ResizeVertical,
        egui::CursorIcon::ResizeNeSw
        | egui::CursorIcon::ResizeNorthEast
        | egui::CursorIcon::ResizeSouthWest => CursorIcon::ResizeNeSw,
        egui::CursorIcon::ResizeNwSe
        | egui::CursorIcon::ResizeNorthWest
        | egui::CursorIcon::ResizeSouthEast => CursorIcon::ResizeNwSe,
        _ => CursorIcon::Default,
    }
}

fn pointer_button(button: PointerButton) -> egui::PointerButton {
    match button {
        PointerButton::Primary => egui::PointerButton::Primary,
        PointerButton::Secondary => egui::PointerButton::Secondary,
        PointerButton::Middle => egui::PointerButton::Middle,
        PointerButton::Back => egui::PointerButton::Extra1,
        PointerButton::Forward | PointerButton::Other(_) => egui::PointerButton::Extra2,
    }
}

fn wheel_unit(unit: WheelUnit) -> egui::MouseWheelUnit {
    match unit {
        WheelUnit::Pixels => egui::MouseWheelUnit::Point,
        WheelUnit::Lines => egui::MouseWheelUnit::Line,
        WheelUnit::Pages => egui::MouseWheelUnit::Page,
    }
}

#[cfg(test)]
mod tests;
