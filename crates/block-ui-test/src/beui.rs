use beui::{
    Color32, Document, Event, Key, Modifiers, PointerButton, Pos2, Rect, TouchId, TouchPhase, Vec2,
};
use block_editor_beui::be_block::LiveEdit;
use block_editor_beui::headless::{Adopted, HeadlessPlugin};
use block_editor_beui::{
    Artifacts, BeuiApp, ChildPlacement, ChildStatus, Creation, Editor, EditorHost,
    EditorInstanceId, EditorRegion, HostReply, HostRequest, Occluder, PeerPresence, SeededContent,
    ShownPresence, ViewChange, WebViewCommand,
};
use block_plugin_api::{
    BlockTypeDescriptor, ChildRect, EditorMessage, FrameChrome, FrameReport, HelloAccepted,
    InputBatch, Message, PROTOCOL_VERSION, ScreenId, ScreenRequest, ScreenSet, SurfaceFormat,
    SurfaceSpec, Theme, ViewportMetrics,
};
use std::marker::PhantomData;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use uuid::Uuid;

use crate::input::Input;
use crate::{ContentStore, snapshot};

mod capture;

const SIZE: Vec2 = Vec2::new(800.0, 600.0);
const EAGER_FRAMES: usize = 8;
const SETTLE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
const HOST_ROUNDS: usize = 8;
const MINIMUM_ZOOM: f32 = 1.0 / 64.0;
const MAXIMUM_ZOOM: f32 = 32.0;
const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const SCREEN: ScreenId = ScreenId(1);
const SURFACE_SIDE: u32 = 8192;

pub struct BeuiTest<A: BeuiApp> {
    plugin: HeadlessPlugin,
    kind: Kind,
    host: EditorHost,
    store: ContentStore,
    size: Vec2,
    scale_factor: f32,
    frame: Option<block_plugin_api::FrameSpec>,
    input: Input,
    frames: Vec<Vec<Event>>,
    inbox: Vec<Message>,
    sent: Vec<EditorMessage>,
    output: Option<beui::FrameOutput>,
    recording: Option<snapshot::Snapshot>,
    viewport: Option<Viewport>,
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    report: Option<FrameReport>,
    intrinsic: Option<Vec2>,
    exited: bool,
    draft: Vec<u8>,
    next_request: u64,
    screens: u64,
    wakes: Arc<Wakes>,
    app: PhantomData<A>,
}

#[derive(Default)]
struct Wakes {
    count: Mutex<u64>,
    woken: Condvar,
}

impl Wakes {
    fn count(&self) -> u64 {
        *self.count.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wake(&self) {
        *self.count.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        self.woken.notify_all();
    }

    fn wait_past(&self, seen: u64, deadline: std::time::Duration) {
        let count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        drop(
            self.woken
                .wait_timeout_while(count, deadline, |count| *count == seen)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Frame(Uuid),
    Preview(Uuid),
    Creation,
    Settings,
}

impl<A: BeuiApp> BeuiTest<A> {
    pub fn new(editor: Editor) -> Self {
        let block = editor.block_id();
        Self::open(
            Kind::Frame(block),
            Adopted::Editor(editor, None),
            Vec::new(),
        )
    }

    pub fn with_view(editor: Editor, view: impl FnOnce() -> beui::NodeId + 'static) -> Self {
        let block = editor.block_id();
        Self::open(
            Kind::Frame(block),
            Adopted::Editor(editor, Some(Box::new(view))),
            Vec::new(),
        )
    }

    pub fn preview(editor: Editor) -> Self {
        let block = editor.block_id();
        Self::open(Kind::Preview(block), Adopted::Preview(editor), Vec::new())
    }

    pub fn creation(creation: Creation) -> Self {
        Self::open(Kind::Creation, Adopted::Creation(creation), Vec::new())
    }

    pub fn settings(artifacts: Artifacts, data: Vec<u8>) -> Self {
        Self::open(Kind::Settings, Adopted::Artifacts(artifacts), data)
    }

    fn open(kind: Kind, adopted: Adopted, data: Vec<u8>) -> Self {
        let host = match &adopted {
            Adopted::Editor(editor, _) | Adopted::Preview(editor) => editor.host().clone(),
            Adopted::Creation(creation) => creation.host().clone(),
            Adopted::Artifacts(artifacts) => artifacts.host().clone(),
        };
        let template = match &adopted {
            Adopted::Creation(creation) => creation.template().to_owned(),
            _ => String::new(),
        };
        let artifact = match &adopted {
            Adopted::Artifacts(artifacts) => Some(artifacts.block_id()),
            _ => None,
        };
        let wakes = Arc::new(Wakes::default());
        let woken = Arc::clone(&wakes);
        host.waker().install(move || woken.wake());
        let mut plugin = HeadlessPlugin::new("block-ui-test", "block-ui-test", "0");
        plugin.adopt::<A>(INSTANCE, adopted);
        let store = ContentStore::new(host.workspace_id());
        let block_type = host.block_type().unwrap_or_default();
        let (account_id, workspace_id, client_id) = (
            host.account_id().into_bytes(),
            host.workspace_id().into_bytes(),
            host.client_id().into_bytes(),
        );
        let open = match kind {
            Kind::Frame(block) | Kind::Preview(block) => {
                store.own(block, block_type);
                EditorMessage::Open {
                    instance: INSTANCE,
                    block_id: block.into_bytes(),
                    block_type: block_type.into_bytes(),
                    account_id,
                    workspace_id,
                    client_id,
                    editable: host.editable(),
                }
            }
            Kind::Creation => EditorMessage::OpenCreation {
                instance: INSTANCE,
                block_type: block_type.into_bytes(),
                template: template.clone(),
                account_id,
                workspace_id,
                client_id,
            },
            Kind::Settings => EditorMessage::OpenArtifact {
                instance: INSTANCE,
                source_type: block_type.into_bytes(),
                block_id: artifact.unwrap_or_default().into_bytes(),
                block_type: block_type.into_bytes(),
                account_id,
                workspace_id,
                client_id,
                data: data.clone(),
            },
        };
        let frame = matches!(kind, Kind::Frame(_)).then(|| block_plugin_api::FrameSpec {
            chrome: FrameChrome::Drawn,
            content: None,
            top_bar: false,
        });
        let mut test = Self {
            plugin,
            kind,
            host,
            store,
            size: SIZE,
            scale_factor: 1.0,
            frame,
            input: Input::default(),
            frames: Vec::new(),
            inbox: Vec::new(),
            sent: Vec::new(),
            output: None,
            recording: None,
            viewport: None,
            children: Vec::new(),
            occluders: Vec::new(),
            report: None,
            intrinsic: None,
            exited: false,
            draft: data,
            next_request: 0,
            screens: 0,
            wakes,
            app: PhantomData,
        };
        let hello = test.plugin.hello();
        test.deliver(roundtrip(hello, "the plugin's hello"));
        test.deliver(Message::HelloAccepted(HelloAccepted {
            version: PROTOCOL_VERSION,
            host_name: "block-ui-test".to_owned(),
            surface: Some(SurfaceSpec {
                format: SurfaceFormat::Rgba8UnormSrgb,
                max_side: SURFACE_SIDE,
            }),
            theme: Theme::default(),
        }));
        test.deliver(Message::Editor(open));
        test.place();
        test.run();
        test
    }

    fn deliver(&mut self, message: Message) {
        if matches!(message, Message::Hello(_)) {
            return;
        }
        let message = roundtrip(message, "a message the host sent");
        for reply in self.plugin.receive(message) {
            if let Message::Error(error) = reply {
                panic!("the plugin refused what the host sent: {}", error.message);
            }
        }
    }

    fn place(&mut self) {
        self.screens += 1;
        let region = self.region();
        let metrics = ViewportMetrics {
            logical_width: self.size.x,
            logical_height: self.size.y,
            visible_x: 0.0,
            visible_y: 0.0,
            pixel_width: (self.size.x * self.scale_factor).round() as u32,
            pixel_height: (self.size.y * self.scale_factor).round() as u32,
            scale_factor: self.scale_factor,
        };
        self.inbox.push(Message::Screens(ScreenSet {
            request_id: self.screens,
            screens: vec![ScreenRequest {
                screen: SCREEN,
                instance: INSTANCE,
                region,
                metrics,
                frame: self.frame.clone(),
            }],
        }));
    }

    pub fn block_id(&self) -> Option<Uuid> {
        match self.kind {
            Kind::Frame(block) | Kind::Preview(block) => Some(block),
            Kind::Creation | Kind::Settings => None,
        }
    }

    pub fn host(&self) -> &EditorHost {
        &self.host
    }

    pub fn store(&self) -> ContentStore {
        self.store.clone()
    }

    pub fn hold<C: LiveEdit>(&mut self, block: Option<Uuid>, content: C) {
        self.store.hold(block, content);
    }

    pub fn content<C: LiveEdit + Clone>(&self, block: Option<Uuid>) -> C {
        self.store.content(block)
    }

    pub fn applied(&self, block: Option<Uuid>) -> u64 {
        self.store.applied(block)
    }

    pub fn holds(&self, block: Option<Uuid>) -> bool {
        self.store.holds(block)
    }

    pub fn edit<C: LiveEdit>(&mut self, block: Option<Uuid>, operation: &C::Op) {
        self.store.edit::<C>(block, operation);
        self.run();
    }

    pub fn seeded(&self) -> Vec<SeededContent> {
        self.store.seeded()
    }

    pub fn draft(&self) -> &[u8] {
        &self.draft
    }

    pub fn set_draft(&mut self, data: Vec<u8>) {
        self.draft.clone_from(&data);
        self.inbox
            .push(Message::Editor(EditorMessage::ArtifactSettings {
                instance: INSTANCE,
                data,
            }));
    }

    pub fn with_top_bar(mut self, closable: bool) -> Self {
        self.frame = Some(block_plugin_api::FrameSpec {
            chrome: FrameChrome::Drawn,
            content: closable.then_some(ChildRect {
                x: 0.0,
                y: 0.0,
                width: self.size.x,
                height: self.size.y,
            }),
            top_bar: true,
        });
        self.place();
        self.run();
        self
    }

    pub fn set_chrome(&mut self, drawn: bool) {
        let frame = self
            .frame
            .as_mut()
            .expect("only an editor's frame has chrome");
        frame.chrome = match drawn {
            true => FrameChrome::Drawn,
            false => FrameChrome::None,
        };
        self.place();
        self.run();
    }

    pub fn set_view(&mut self, view: Rect, scale: f32) {
        let view = view.translate(-self.origin());
        self.inbox.push(Message::Editor(EditorMessage::ViewChanged {
            instance: INSTANCE,
            x: view.min.x,
            y: view.min.y,
            width: view.width(),
            height: view.height(),
            scale,
        }));
    }

    pub fn drag_block(&mut self, position: Pos2, block_id: Uuid, block_type: Uuid, dropped: bool) {
        let position = position - self.origin();
        self.inbox.push(Message::Editor(EditorMessage::DragOver {
            instance: INSTANCE,
            region: self.region(),
            x: position.x,
            y: position.y,
            block_id: block_id.into_bytes(),
            block_type: block_type.into_bytes(),
            dropped,
        }));
    }

    pub fn with_scale_factor(mut self, scale_factor: f32) -> Self {
        self.scale_factor = scale_factor;
        self.place();
        self.run();
        self
    }

    pub fn block_types(&mut self, descriptors: Vec<BlockTypeDescriptor>) {
        self.inbox.push(Message::BlockTypes(descriptors));
        self.run();
    }

    pub fn exited(&self) -> bool {
        self.exited
    }

    pub fn in_viewport(mut self) -> Self {
        self.viewport = Some(Viewport::new());
        self.run();
        self
    }

    pub fn wants_another_frame(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| output.changed || output.repaint_after < std::time::Duration::MAX)
    }

    pub fn document(&self) -> &Document {
        self.plugin
            .document(INSTANCE, self.region())
            .expect("the editor has not built its view yet")
    }

    pub fn children(&self) -> &[ChildPlacement] {
        &self.children
    }

    pub fn occluders(&self) -> &[Occluder] {
        &self.occluders
    }

    fn band(&self, rect: Rect) -> Rect {
        self.content_rect()
            .filter(|content| content.is_positive())
            .unwrap_or(rect)
    }

    pub fn content_rect(&self) -> Option<Rect> {
        let report = self.report.as_ref()?;
        let origin = self.origin();
        let rect = report.content;
        Some(Rect::from_min_size(
            Pos2::new(rect.x + origin.x, rect.y + origin.y),
            Vec2::new(rect.width, rect.height),
        ))
    }

    pub fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        self.next_request += 1;
        let request_id = self.next_request;
        self.inbox
            .push(Message::Editor(EditorMessage::ReplaceChild {
                instance: INSTANCE,
                request_id,
                old: old.into_bytes(),
                new: new.into_bytes(),
            }));
        self.step(Vec::new());
        let mut replaced = None;
        self.sent.retain(|message| match message {
            EditorMessage::ChildReplaced {
                request_id: answered,
                replaced: outcome,
                ..
            } if *answered == request_id => {
                replaced = Some(*outcome);
                false
            }
            _ => true,
        });
        replaced.expect("the plugin never answered the replacement")
    }

    pub fn presence_visible(&mut self, visible: bool) {
        self.inbox.push(Message::Editor(EditorMessage::Presence {
            instance: INSTANCE,
            visible,
        }));
        self.run();
    }

    pub fn resize(&mut self, size: Vec2) {
        self.inbox.push(Message::Editor(EditorMessage::Resized {
            instance: INSTANCE,
            width: size.x,
            height: size.y,
        }));
        self.run();
    }

    pub fn set_histories(
        &mut self,
        states: impl IntoIterator<Item = (Uuid, block_editor_beui::BlockHistory)>,
    ) {
        self.inbox
            .push(Message::Editor(EditorMessage::HistoryStates {
                instance: INSTANCE,
                states: states
                    .into_iter()
                    .map(|(block, history)| block_plugin_api::HistoryState {
                        block_id: block.into_bytes(),
                        can_undo: history.can_undo,
                        can_redo: history.can_redo,
                    })
                    .collect(),
            }));
    }

    pub fn set_version_status(&mut self, status: block_editor_beui::VersionStatus) {
        let block = self
            .block_id()
            .expect("a version status is about the editor's own block");
        self.inbox
            .push(Message::Editor(EditorMessage::VersionStatus {
                instance: INSTANCE,
                block_id: block.into_bytes(),
                status,
            }));
    }

    pub fn set_peers(&mut self, block: Option<Uuid>, peers: Vec<PeerPresence>) {
        let block = block
            .or(self.block_id())
            .expect("this editor has no block of its own; name the block");
        self.inbox
            .push(Message::Editor(EditorMessage::PeerPresence {
                instance: INSTANCE,
                block_id: block.into_bytes(),
                peers: peers
                    .into_iter()
                    .map(|peer| block_plugin_api::PeerPresence {
                        client: peer.client,
                        kind: peer.kind.into_bytes(),
                        value: peer.value,
                    })
                    .collect(),
            }));
    }

    pub fn web_view_event(&mut self, event: block_editor_beui::WebViewEvent) {
        self.inbox
            .push(Message::Editor(EditorMessage::WebViewEvent {
                instance: INSTANCE,
                event,
            }));
    }

    pub fn reply(&mut self, request_id: u64, reply: HostReply) {
        self.inbox.push(Message::Editor(EditorMessage::Replied {
            instance: INSTANCE,
            request_id,
            reply,
        }));
    }

    pub fn report_children(&mut self, report: impl Fn(&ChildPlacement) -> ChildStatus) {
        let statuses: Vec<ChildStatus> = self
            .children
            .iter()
            .map(report)
            .map(|status| ChildStatus {
                instance: INSTANCE,
                ..status
            })
            .collect();
        self.inbox.push(Message::ChildStatuses(statuses));
    }

    pub fn available_children(&mut self) {
        let region = self.region();
        self.report_children(|placement| ChildStatus {
            instance: INSTANCE,
            region,
            child: placement.child,
            available: true,
            intrinsic: None,
            aspect_ratio: None,
            hovered: false,
            active: false,
            interaction: block_editor_beui::InteractionMode::Preview,
            capabilities: block_editor_beui::EditorCapabilities::default(),
            resize: block_editor_beui::ResizeMode::None,
            error: None,
        });
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO + self.origin(), self.size)
    }

    fn origin(&self) -> Vec2 {
        self.plugin
            .layout()
            .placement(SCREEN)
            .map_or(Vec2::ZERO, |placement| {
                let scale = placement.scale_factor();
                Vec2::new(placement.x as f32 / scale, placement.y as f32 / scale)
            })
    }

    pub fn run(&mut self) {
        let frames = std::mem::take(&mut self.frames);
        if frames.is_empty() {
            self.step(Vec::new());
        }
        for events in frames {
            self.step(events);
        }
        for _ in 0..HOST_ROUNDS {
            if self.inbox.is_empty() && !self.store.pending() {
                break;
            }
            self.step(Vec::new());
        }
    }

    pub fn settle_until(&mut self, what: &str, ready: impl Fn(&Self) -> bool) {
        let started = std::time::Instant::now();
        let mut eager = 0;
        while started.elapsed() < SETTLE_DEADLINE {
            let seen = self.wakes.count();
            self.run();
            if ready(self) {
                return;
            }
            let (changed, requested) = self
                .output
                .as_ref()
                .map_or((false, std::time::Duration::MAX), |output| {
                    (output.changed, output.repaint_after)
                });
            if changed && eager < EAGER_FRAMES {
                eager += 1;
                continue;
            }
            eager = 0;
            let left = SETTLE_DEADLINE.saturating_sub(started.elapsed());
            self.wakes.wait_past(seen, requested.min(left));
        }
        panic!("the editor was still waiting for {what} after {SETTLE_DEADLINE:?}");
    }

    pub fn step(&mut self, events: Vec<Event>) {
        let mut inbox = std::mem::take(&mut self.inbox);
        inbox.extend(self.store.outgoing(INSTANCE));
        let rect = self.rect();
        let band = self.band(rect);
        let intrinsic = self.intrinsic;
        if let Some(message) = self
            .viewport
            .as_mut()
            .and_then(|viewport| viewport.place(band, rect.min, intrinsic))
        {
            inbox.push(message);
        }
        let events = self.input.normalize(events, self.origin());
        if !events.is_empty() {
            inbox.push(Message::Input(InputBatch {
                screen: SCREEN,
                events,
            }));
        }
        inbox.push(Message::DrawFrame);
        for message in inbox {
            self.deliver(message);
        }
        let output = self
            .plugin
            .draw()
            .into_iter()
            .find(|(placement, _)| placement.screen == SCREEN)
            .map(|(_, output)| output);
        if output.is_some() {
            self.output = output;
        }
        for message in self.plugin.outbound() {
            self.handle(roundtrip(message, "a message the plugin sent"));
        }
        let intrinsic = self.intrinsic;
        let band = self.band(rect);
        if let Some(viewport) = &mut self.viewport {
            viewport.settle(&mut self.sent, band, rect.min);
            self.inbox.extend(viewport.place(band, rect.min, intrinsic));
        }
    }

    fn handle(&mut self, message: Message) {
        match message {
            Message::Children(placements) => {
                self.children = placements.children;
                self.occluders = placements.occluders;
            }
            Message::Frames(reports) => {
                if let Some(report) = reports.into_iter().find(|report| report.screen == SCREEN) {
                    self.report = Some(report);
                }
            }
            Message::Editor(message) => {
                self.store.receive(&message);
                match &message {
                    EditorMessage::LeaveFrame { .. } => self.exited = true,
                    EditorMessage::IntrinsicSize { size, .. } => {
                        self.intrinsic = size.map(|size| Vec2::new(size.width, size.height));
                    }
                    EditorMessage::ArtifactEdited { data, .. } => self.draft.clone_from(data),
                    _ => {}
                }
                self.sent.push(message);
            }
            Message::Error(error) => panic!("the plugin reported an error: {}", error.message),
            _ => {}
        }
    }

    pub fn sent(&self) -> &[EditorMessage] {
        &self.sent
    }

    pub fn take_sent(&mut self) -> Vec<EditorMessage> {
        std::mem::take(&mut self.sent)
    }

    fn take_where<T>(&mut self, pick: impl Fn(&EditorMessage) -> Option<T>) -> Vec<T> {
        let mut taken = Vec::new();
        self.sent.retain(|message| match pick(message) {
            Some(value) => {
                taken.push(value);
                false
            }
            None => true,
        });
        taken
    }

    pub fn take_view_changes(&mut self) -> Vec<ViewChange> {
        self.take_where(|message| match message {
            EditorMessage::ChangeView { change, .. } => Some(*change),
            _ => None,
        })
    }

    pub fn take_requests(&mut self) -> Vec<(u64, HostRequest)> {
        self.take_where(|message| match message {
            EditorMessage::Request {
                request_id,
                request,
                ..
            } => Some((*request_id, request.clone())),
            _ => None,
        })
    }

    pub fn take_opens(&mut self) -> Vec<(Uuid, Uuid, Option<Uuid>)> {
        self.take_where(|message| match message {
            EditorMessage::OpenBlock {
                block_id,
                block_type,
                via,
                ..
            } => Some((
                Uuid::from_bytes(*block_id),
                Uuid::from_bytes(*block_type),
                via.map(Uuid::from_bytes),
            )),
            _ => None,
        })
    }

    pub fn take_block_commands(&mut self) -> Vec<(Uuid, block_editor_beui::BlockCommand)> {
        self.take_where(|message| match message {
            EditorMessage::BlockCommand {
                block_id, command, ..
            } => Some((Uuid::from_bytes(*block_id), *command)),
            _ => None,
        })
    }

    pub fn take_version_commands(&mut self) -> Vec<(Uuid, block_editor_beui::VersionCommand)> {
        self.take_where(|message| match message {
            EditorMessage::VersionControl {
                block_id, command, ..
            } => Some((Uuid::from_bytes(*block_id), command.clone())),
            _ => None,
        })
    }

    pub fn take_shown_presence(&mut self) -> Vec<ShownPresence> {
        self.take_where(|message| match message {
            EditorMessage::ShowPresence {
                block_id,
                kind,
                value,
                ..
            } => Some(ShownPresence {
                block: Some(Uuid::from_bytes(*block_id)),
                kind: Uuid::from_bytes(*kind),
                value: value.as_ref().map(|value| value.to_vec()),
            }),
            _ => None,
        })
    }

    pub fn take_web_view_commands(&mut self) -> Vec<WebViewCommand> {
        self.take_where(|message| match message {
            EditorMessage::WebViewCommand { command, .. } => Some(command.clone()),
            _ => None,
        })
    }

    pub fn take_drag_accepted(&mut self) -> Option<bool> {
        self.take_where(|message| match message {
            EditorMessage::DragAccepted { accepted, .. } => Some(*accepted),
            _ => None,
        })
        .pop()
    }

    pub fn take_cursor_grab(&mut self) -> Option<bool> {
        self.take_where(|message| match message {
            EditorMessage::GrabCursor { grabbed, .. } => Some(*grabbed),
            _ => None,
        })
        .pop()
    }

    pub fn pointer_locked(&self) -> bool {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .pointer_locked
    }

    pub fn pointer_motion(&mut self, delta: Vec2) {
        self.push(Event::PointerMotion(delta));
    }

    pub fn intrinsic_size(&self) -> Option<Vec2> {
        self.intrinsic
    }

    fn region(&self) -> EditorRegion {
        match self.kind {
            Kind::Preview(_) => EditorRegion::Preview,
            Kind::Settings => EditorRegion::ArtifactSettings,
            Kind::Frame(_) | Kind::Creation => EditorRegion::Frame,
        }
    }

    fn push(&mut self, event: Event) {
        match self.frames.last_mut() {
            Some(frame) => frame.push(event),
            None => self.frames.push(vec![event]),
        }
    }

    pub fn next_frame(&mut self) {
        if self.frames.last().is_some_and(|frame| !frame.is_empty()) {
            self.frames.push(Vec::new());
        }
    }

    pub fn hover_at(&mut self, pos: Pos2) {
        self.push(Event::PointerMoved(pos));
    }

    pub fn shown(&self, test_id: &str) -> bool {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_id_rect(test_id)
            .is_some()
    }

    pub fn label(&self, test_id: &str) -> String {
        let node = self
            .document()
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
        let mut collected = Vec::new();
        collect_text(self.document(), node, &mut collected);
        collected.join(" ")
    }

    pub fn rect_of(&self, test_id: &str) -> Rect {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_id_rect(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"))
    }

    pub fn hover(&mut self, test_id: &str) {
        self.hover_at(self.rect_of(test_id).center());
    }

    pub fn click(&mut self, test_id: &str) {
        self.click_at(self.rect_of(test_id).center());
    }

    pub fn double_click(&mut self, test_id: &str) {
        self.double_click_at(self.rect_of(test_id).center());
    }

    pub fn double_click_at(&mut self, pos: Pos2) {
        self.click_at(pos);
        self.click_at(pos);
    }

    pub fn click_at(&mut self, pos: Pos2) {
        let modifiers = self.input.held();
        self.hover_at(pos);
        self.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers,
        });
        self.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers,
        });
    }

    pub fn drag(&mut self, from: Pos2, to: Pos2) {
        let modifiers = self.input.held();
        self.hover_at(from);
        self.push(Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers,
        });
        self.next_frame();
        self.push(Event::PointerMoved(to));
        self.next_frame();
        self.push(Event::PointerMoved(to + Vec2::new(0.0, 0.5)));
        self.push(Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers,
        });
    }

    pub fn secondary_drag(&mut self, from: Pos2, to: Pos2) {
        let modifiers = self.input.held();
        self.hover_at(from);
        self.push(Event::PointerButton {
            pos: from,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers,
        });
        self.next_frame();
        self.push(Event::PointerMoved(to));
        self.push(Event::PointerButton {
            pos: to,
            button: PointerButton::Secondary,
            pressed: false,
            modifiers,
        });
    }

    pub fn touch_start(&mut self, pos: Pos2) {
        self.touch(TouchPhase::Start, pos);
    }

    pub fn touch_move(&mut self, pos: Pos2) {
        self.touch(TouchPhase::Move, pos);
    }

    pub fn touch_end(&mut self, pos: Pos2) {
        self.touch(TouchPhase::End, pos);
    }

    pub fn touch_cancel(&mut self, pos: Pos2) {
        self.touch(TouchPhase::Cancel, pos);
    }

    fn touch(&mut self, phase: TouchPhase, pos: Pos2) {
        self.finger(1, phase, pos);
    }

    pub fn finger(&mut self, finger: u64, phase: TouchPhase, pos: Pos2) {
        self.push(Event::Touch {
            id: TouchId { device: 1, finger },
            phase,
            pos,
            force: None,
        });
    }

    pub fn key_press(&mut self, key: Key) {
        self.key_press_modifiers(Modifiers::NONE, key);
    }

    pub fn key_press_modifiers(&mut self, modifiers: Modifiers, key: Key) {
        let held = self.input.held();
        if modifiers != held {
            self.push(Event::Modifiers(modifiers));
        }
        self.push(Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
        });
        self.push(Event::Key {
            key,
            pressed: false,
            repeat: false,
            modifiers,
        });
        if modifiers != held {
            self.push(Event::Modifiers(held));
        }
    }

    pub fn text(&mut self, text: impl Into<String>) {
        self.push(Event::Text(text.into()));
    }

    pub fn record(&mut self) {
        let frame = self.painted();
        match &mut self.recording {
            Some(recording) => recording.append(frame),
            None => self.recording = Some(frame),
        }
    }

    pub fn snapshot(&mut self, name: &str) {
        let painting = match self.recording.take() {
            Some(recording) => recording,
            None => self.painted(),
        };
        snapshot::assert_snapshot(name, &painting);
    }

    fn painted(&mut self) -> snapshot::Snapshot {
        let output = self
            .output
            .as_ref()
            .expect("the editor has not drawn a frame yet");
        capture::capture(output, self.size, output.pixels_per_point(), Color32::BLACK)
            .expect("the painting could not be rendered")
    }
}

fn roundtrip(message: Message, what: &str) -> Message {
    let frame = block_plugin_api::encode_frame(&message)
        .unwrap_or_else(|error| panic!("{what} could not be encoded: {error:?}"));
    block_plugin_api::decode_frame(&frame)
        .unwrap_or_else(|error| panic!("{what} could not be read back: {error:?}"))
}

fn collect_text(document: &Document, node: beui::NodeId, collected: &mut Vec<String>) {
    if document.node_kind(node) == "text" {
        let detail = document.node_detail(node).unwrap_or_default();
        let text = detail.trim_matches('"');
        if !text.is_empty() {
            collected.push(text.to_owned());
        }
    }
    for child in document.children(node) {
        collect_text(document, child, collected);
    }
}

struct Viewport {
    zoom: f32,
    pan: Vec2,
    fitting: bool,
    sent: Option<(Rect, f32)>,
}

impl Viewport {
    fn new() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            fitting: true,
            sent: None,
        }
    }

    fn place(&mut self, region: Rect, origin: Pos2, intrinsic: Option<Vec2>) -> Option<Message> {
        let content = intrinsic
            .unwrap_or(Vec2::ZERO)
            .max(region.size())
            .max(Vec2::new(1.0, 1.0));
        if self.fitting {
            self.zoom = (region.width() / content.x)
                .min(region.height() / content.y)
                .min(1.0)
                .clamp(MINIMUM_ZOOM, MAXIMUM_ZOOM);
            self.pan = Vec2::ZERO;
        }
        let size = content * self.zoom;
        let center = region.center() + self.pan;
        let view = Rect::from_min_size(center - size * 0.5, size).translate(-origin.to_vec2());
        if self.sent == Some((view, self.zoom)) {
            return None;
        }
        self.sent = Some((view, self.zoom));
        Some(Message::Editor(EditorMessage::ViewChanged {
            instance: INSTANCE,
            x: view.min.x,
            y: view.min.y,
            width: view.width(),
            height: view.height(),
            scale: self.zoom,
        }))
    }

    fn settle(&mut self, sent: &mut Vec<EditorMessage>, region: Rect, origin: Pos2) {
        let mut changes = Vec::new();
        sent.retain(|message| match message {
            EditorMessage::ChangeView { change, .. } => {
                changes.push(*change);
                false
            }
            _ => true,
        });
        for change in changes {
            if change != ViewChange::ResumeAutoFit {
                self.fitting = false;
            }
            match change {
                ViewChange::Pan { x, y } => self.pan += Vec2::new(x, y),
                ViewChange::Zoom { factor, anchor } => {
                    let zoom = (self.zoom * factor).clamp(MINIMUM_ZOOM, MAXIMUM_ZOOM);
                    let anchor = anchor
                        .map_or(region.center(), |(x, y)| Pos2::new(x, y) + origin.to_vec2())
                        - region.center();
                    self.pan = anchor - (anchor - self.pan) * (zoom / self.zoom);
                    self.zoom = zoom;
                }
                ViewChange::Fit | ViewChange::ResumeAutoFit => self.fitting = true,
            }
        }
    }
}
