use beui::testing::{Target, button_name};
use beui::{
    Document, Input, Key, KeyChord, Modifiers, Pen, PenPhase, PointerButton, Pos2, Rect,
    TouchPhase, Vec2,
};
use block_editor_beui::FromEditor;
use block_editor_beui::be_block::{LiveEdit, Merge};
use block_editor_beui::headless::{Adopted, HeadlessPlugin};
use block_editor_beui::{
    Artifacts, BeuiApp, ChildPlacement, ChildStatus, Creation, EditorHost, EditorInstanceId,
    EditorRegion, Occluder, OwnedEditor, PeerPresence, SeededContent, ShownPresence, ViewChange,
    WebViewCommand, WebViewId,
};
use block_plugin_api::Opening;
use block_plugin_api::embed::blocks::Call as BlocksCall;
use block_plugin_api::embed::debug::debug::Call as DebugCall;
use block_plugin_api::embed::{exports::plugin::Editor, shell::Call as ShellCall};
use block_plugin_api::ops::{Connection, Incoming, Side};
use block_plugin_api::{
    BarAction, BlockTypeDescriptor, Catalog, ChildId, FitArea, FrameChrome, FrameCommit,
    FrameReport, HelloAccepted, InputBatch, MenuCommand, MenuEntry, OpenArtifact, OpenBlock,
    OpenCreation, ScreenId, ScreenRequest, SurfaceFormat, SurfaceSpec, TimedInput, TopBar,
    ViewportMetrics,
};
use block_plugin_api::{
    embed::host_services::Call,
    ops::{OpError, OpId, Outcome, Reply},
};
use std::marker::PhantomData;

type Delivered = Box<dyn FnOnce(&mut HeadlessPlugin) -> Result<(), String>>;

enum Inbound {
    Pushed(Delivered),
    Frames(Vec<Vec<u8>>),
}

impl Inbound {
    fn pushed(deliver: impl FnOnce(&mut HeadlessPlugin) -> Result<(), String> + 'static) -> Self {
        Self::Pushed(Box::new(deliver))
    }
}

pub type HostCall = Call;

use std::sync::{Arc, Condvar, Mutex, PoisonError};
use uuid::Uuid;

use crate::document::FRAME_INTERVAL;
use crate::link::Sent;
use crate::{ContentStore, overflow, snapshot};
use beui::drive::gestures::{Frames, Gestures};

pub(crate) mod capture;

const SIZE: Vec2 = Vec2::new(800.0, 600.0);
const EAGER_FRAMES: usize = 8;
const SETTLE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
const HOST_ROUNDS: usize = 8;
const MINIMUM_ZOOM: f32 = 1.0 / 64.0;
const MAXIMUM_ZOOM: f32 = 32.0;
const INSTANCE: EditorInstanceId = EditorInstanceId(1);

const ADOPTED_EDITOR: [u8; 16] = [0; 16];
const SCREEN: ScreenId = ScreenId(1);
const WALL_CLOCK: std::time::Duration = std::time::Duration::from_secs(1_772_444_460);

pub struct BeuiTest<A: BeuiApp> {
    plugin: HeadlessPlugin,
    kind: Kind,
    host: EditorHost,
    store: ContentStore,
    size: Vec2,
    scale_factor: f32,
    frame: Option<block_plugin_api::FrameSpec>,
    gestures: Gestures,
    steps: Vec<String>,
    frames: Vec<Vec<Input>>,
    inbox: Vec<Inbound>,
    connection: Connection,
    link: crate::link::Link,
    editor: Option<Editor>,
    calls: Vec<ShellCall>,
    extension_calls: Vec<block_plugin_api::ops::Called>,
    web_views: std::collections::HashMap<
        block_plugin_api::ops::OpId,
        (
            WebViewId,
            block_plugin_api::Watch<Vec<block_editor_beui::WebViewEvent>>,
        ),
    >,
    web_view_commands: Vec<(WebViewId, WebViewCommand)>,
    published: std::collections::HashMap<block_plugin_api::ops::MethodId, Vec<u8>>,
    published_watches: std::collections::HashMap<
        block_plugin_api::ops::OpId,
        (
            block_plugin_api::ops::MethodId,
            block_plugin_api::Watch<block_plugin_api::Encoded>,
        ),
    >,
    asked_picks: Vec<(
        u64,
        block_plugin_api::ops::Ticket<block_plugin_api::BlockPick>,
    )>,
    child_statuses: block_plugin_api::Watch<Vec<ChildStatus>>,
    block_types: block_plugin_api::Watch<Catalog>,
    debug_templates: block_plugin_api::Watch<Vec<block_plugin_api::DebugTemplate>>,
    completed:
        std::collections::HashMap<block_plugin_api::ops::OpId, block_plugin_api::ops::Outcome>,
    menu: Vec<MenuEntry>,
    output: Option<beui::FrameOutput>,
    shown_window: Option<beui::WindowId>,
    drew: bool,
    recording: Option<snapshot::Snapshot>,
    overflows: Vec<String>,
    viewport: Option<Viewport>,
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    report: Option<FrameReport>,
    window: block_plugin_api::WindowRequests,
    intrinsic: Option<Vec2>,
    exited: bool,
    draft: Vec<u8>,
    requests: Vec<(Option<OpId>, Call)>,
    wakes: Arc<Wakes>,
    clock: std::time::Duration,
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
    pub fn new(editor: OwnedEditor) -> Self {
        let block = editor.block_id();
        Self::open(
            Kind::Frame(block),
            Adopted::Editor(editor, None),
            Vec::new(),
        )
    }

    pub fn with_view(
        editor: OwnedEditor,
        view: impl FnOnce() -> beui::styled::Root + 'static,
    ) -> Self {
        let block = editor.block_id();
        Self::open(
            Kind::Frame(block),
            Adopted::Editor(editor, Some(Box::new(view))),
            Vec::new(),
        )
    }

    pub fn preview(editor: OwnedEditor) -> Self {
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
        beui::verify_paint(true);
        beui::fail_unsettled_layout(true);
        block_editor_beui::pin_wall_clock(Some(WALL_CLOCK));
        let host = match &adopted {
            Adopted::Editor(editor, _) | Adopted::Preview(editor) => editor.host(),
            Adopted::Creation(creation) => creation.host().clone(),
            Adopted::Artifacts(artifacts) => artifacts.host().clone(),
        };
        let template = match &adopted {
            Adopted::Creation(creation) => creation.template().to_owned(),
            _ => String::new(),
        };
        let artifact = match &adopted {
            Adopted::Artifacts(artifacts) => Some((artifacts.block_id(), artifacts.source())),
            _ => None,
        };
        let wakes = Arc::new(Wakes::default());
        let woken = Arc::clone(&wakes);
        host.waker().install(move || woken.wake());
        let mut plugin = HeadlessPlugin::new("block-ui-test", "block-ui-test", "0");
        plugin.adopt::<A>(INSTANCE, adopted);
        let store = ContentStore::new(host.organization_id());
        let blocks = host.blocks();
        store.know_elsewhere(move |block| blocks.info(block).is_some());
        let block_type = host.block_type().unwrap_or_default();
        let (account_id, organization_id, client_id) = (
            host.account_id().into_bytes(),
            host.organization_id().into_bytes(),
            host.client_id().into_bytes(),
        );
        let open = match kind {
            Kind::Frame(block) | Kind::Preview(block) => {
                store.own(block, block_type);
                Opening::Block(OpenBlock {
                    instance: INSTANCE,
                    editor: ADOPTED_EDITOR,
                    block_id: block.into_bytes(),
                    block_type: block_type.into_bytes(),
                    view_block: host.view_block().map(Uuid::into_bytes),
                    account_id,
                    organization_id,
                    client_id,
                    editable: host.editable(),
                })
            }
            Kind::Creation => Opening::Creation(OpenCreation {
                instance: INSTANCE,
                editor: ADOPTED_EDITOR,
                template: template.clone(),
                account_id,
                organization_id,
                client_id,
            }),
            Kind::Settings => Opening::Artifact(OpenArtifact {
                instance: INSTANCE,
                editor: ADOPTED_EDITOR,
                block_id: artifact.unwrap_or_default().0.into_bytes(),
                block_type: block_type.into_bytes(),
                source: artifact.unwrap_or_default().1.into_bytes(),
                account_id,
                organization_id,
                client_id,
                data: data.clone(),
            }),
        };
        let frame = match &kind {
            Kind::Frame(_) => Some(block_plugin_api::FrameSpec {
                chrome: FrameChrome::Drawn,
                content: None,
                top_bar: TopBar::Hidden,
                dialog: None,
            }),
            Kind::Creation => Some(block_plugin_api::FrameSpec {
                chrome: FrameChrome::None,
                content: None,
                top_bar: TopBar::Hidden,
                dialog: Some(block_plugin_api::DialogFrame::default()),
            }),
            _ => None,
        };
        let mut test = Self {
            plugin,
            kind,
            host,
            store,
            size: SIZE,
            scale_factor: 1.0,
            frame,
            steps: Vec::new(),
            gestures: Gestures::default(),
            frames: Vec::new(),
            inbox: Vec::new(),
            connection: Connection::new(Side::Host),
            link: crate::link::Link::new(INSTANCE),
            editor: None,
            calls: Vec::new(),
            extension_calls: Vec::new(),
            web_views: std::collections::HashMap::new(),
            web_view_commands: Vec::new(),
            published: std::collections::HashMap::new(),
            published_watches: std::collections::HashMap::new(),
            asked_picks: Vec::new(),
            child_statuses: block_plugin_api::Watch::default(),
            block_types: block_plugin_api::Watch::default(),
            debug_templates: block_plugin_api::Watch::default(),
            completed: std::collections::HashMap::new(),
            menu: Vec::new(),
            output: None,
            shown_window: None,
            drew: false,
            recording: None,
            overflows: Vec::new(),
            viewport: None,
            children: Vec::new(),
            occluders: Vec::new(),
            report: None,
            window: block_plugin_api::WindowRequests::default(),
            intrinsic: None,
            exited: false,
            draft: data,
            requests: Vec::new(),
            wakes,
            clock: std::time::Duration::ZERO,
            app: PhantomData,
        };
        roundtrip(test.plugin.hello(), "the plugin's hello");
        let accepted = roundtrip(
            HelloAccepted {
                host_name: "block-ui-test".to_owned(),
                surface: Some(SurfaceSpec {
                    format: SurfaceFormat::Rgba8UnormSrgb,
                    render: true,
                }),
                preferences: block_plugin_api::Preferences::default(),
            },
            "the host's acceptance",
        );
        test.deliver(Inbound::pushed(move |plugin| plugin.accept(accepted)));
        test.editor = Some(Editor::new(&mut test.connection, open));
        let frames = test.connection.take_outbound();
        test.deliver(Inbound::Frames(frames));
        test.place();
        test.run();
        test
    }

    fn deliver(&mut self, inbound: Inbound) {
        let delivered = match inbound {
            Inbound::Pushed(deliver) => deliver(&mut self.plugin),
            Inbound::Frames(frames) => self.plugin.deliver(frames),
        };
        if let Err(error) = delivered {
            panic!("the plugin refused what the host sent: {error}");
        }
    }

    fn place(&mut self) {
        let region = self.region();
        let metrics = ViewportMetrics {
            logical: block_plugin_api::Size {
                width: self.size.x,
                height: self.size.y,
            },
            visible: block_plugin_api::Pos2::ZERO,
            pixel_width: (self.size.x * self.scale_factor).round() as u32,
            pixel_height: (self.size.y * self.scale_factor).round() as u32,
            scale_factor: self.scale_factor,
        };
        let screens = vec![ScreenRequest {
            screen: SCREEN,
            instance: INSTANCE,
            region,
            metrics,
            frame: self.frame,
        }];
        block_plugin_api::embed::exports::plugin::screens(&mut self.connection, screens);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
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

    pub fn hold<C: LiveEdit + Merge + Clone + Default>(&mut self, block: Option<Uuid>, content: C) {
        self.store.hold(block, content);
    }

    pub fn content<C: LiveEdit + Merge + Clone + Default>(&self, block: Option<Uuid>) -> C {
        self.store.content(block)
    }

    pub fn applied(&self, block: Option<Uuid>) -> u64 {
        self.store.applied(block)
    }

    pub fn holds(&self, block: Option<Uuid>) -> bool {
        self.store.holds(block)
    }

    pub fn edit<C: LiveEdit + Merge + Clone + Default>(
        &mut self,
        block: Option<Uuid>,
        operation: &C::Op,
    ) {
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
        self.call(|editor, connection| editor.artifact_settings(connection, data));
    }

    pub fn with_top_bar(self, closable: bool) -> Self {
        self.with_bar(closable, TopBar::Shown)
    }

    pub fn without_top_bar(self) -> Self {
        self.with_bar(false, TopBar::Hidden)
    }

    pub fn on_phone(self) -> Self {
        self.with_bar(false, TopBar::Phone)
    }

    fn with_bar(mut self, closable: bool, top_bar: TopBar) -> Self {
        self.frame = Some(block_plugin_api::FrameSpec {
            chrome: FrameChrome::Drawn,
            content: closable.then_some(Rect::from_min_size(Pos2::ZERO, self.size)),
            top_bar,
            dialog: None,
        });
        self.place();
        self.run();
        self
    }

    pub fn menu(&self) -> &[MenuEntry] {
        &self.menu
    }

    pub fn menu_entry(&self, id: &str) -> MenuCommand {
        MenuEntry::command(&self.menu, id).unwrap_or_else(|| panic!("the menu offers no {id:?}"))
    }

    pub fn menu_ids(&self) -> Vec<String> {
        fn collect(entries: &[MenuEntry], ids: &mut Vec<String>) {
            for entry in entries {
                match entry {
                    MenuEntry::Command(command) => ids.push(command.id.clone()),
                    MenuEntry::Submenu(submenu) => {
                        ids.push(submenu.id.clone());
                        collect(&submenu.entries, ids);
                    }
                    MenuEntry::Separator => {}
                }
            }
        }
        let mut ids = Vec::new();
        collect(&self.menu, &mut ids);
        ids
    }

    pub fn pick_menu(&mut self, id: &str) {
        self.menu_entry(id);
        let id = id.to_owned();
        self.call(|editor, connection| editor.menu_pick(connection, id));
        self.run();
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

    pub fn dismiss(&mut self) {
        self.call(|editor, connection| editor.dismiss(connection));
        self.run();
    }

    pub fn set_dialog(&mut self, dialog: block_plugin_api::DialogFrame) {
        if let Some(frame) = &mut self.frame {
            frame.dialog = Some(dialog);
        }
        self.place();
        self.run();
    }

    pub fn handles_dismiss(&self) -> Option<bool> {
        self.calls.iter().rev().find_map(|call| match call {
            ShellCall::HandlesDismiss { handles, .. } => Some(*handles),
            _ => None,
        })
    }

    pub fn take_dialog_closes(&mut self) -> Vec<block_plugin_api::DialogAnswer> {
        self.take_calls(|call| match call {
            ShellCall::CloseDialog { answer, .. } => Some(*answer),
            _ => None,
        })
    }

    pub fn take_child_dismissals(&mut self) -> Vec<ChildId> {
        self.take_calls(|call| match call {
            ShellCall::DismissChild { child, .. } => Some(*child),
            _ => None,
        })
    }

    pub fn child_bar(&mut self, child: ChildId, action: BarAction) {
        self.call(|editor, connection| {
            editor.child_bar(connection, EditorRegion::Frame, child, action)
        });
    }

    pub fn set_view(&mut self, view: Rect, scale: f32) {
        let rect = self.rect();
        let band = self.band(rect);
        if let Some(viewport) = &mut self.viewport {
            viewport.show(view, scale, band, rect.min);
        }
        self.call(|editor, connection| editor.view_changed(connection, view, scale));
    }

    pub fn move_view(&mut self, change: ViewChange) {
        let band = self.band(self.rect());
        self.viewport
            .as_mut()
            .expect("only an editor in a viewport has a view to move")
            .apply(change, band);
    }

    pub fn drag_block(&mut self, position: Pos2, block_id: Uuid, block_type: Uuid, dropped: bool) {
        let drag = block_plugin_api::shell_types::DragOver {
            region: self.region(),
            pos: position,
            block_id: block_id.into_bytes(),
            block_type: block_type.into_bytes(),
            dropped,
        };
        self.call(|editor, connection| editor.drag_over(connection, drag));
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.set_size(size);
        self
    }

    pub fn set_size(&mut self, size: Vec2) {
        self.size = size;
        self.place();
        self.run();
    }

    pub fn with_scale_factor(mut self, scale_factor: f32) -> Self {
        self.scale_factor = scale_factor;
        self.place();
        self.run();
        self
    }

    pub fn block_types(&mut self, descriptors: Vec<BlockTypeDescriptor>) {
        self.catalog(Catalog {
            types: descriptors,
            templates: Vec::new(),
            editors: Vec::new(),
        });
    }

    pub fn catalog(&mut self, catalog: Catalog) {
        self.block_types.change(&mut self.connection, |_| catalog);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
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

    pub fn in_moved_viewport(mut self, zoom: f32, pan: Vec2) -> Self {
        self.viewport = Some(Viewport {
            zoom,
            pan,
            moved: true,
            ..Viewport::new()
        });
        self.run();
        self
    }

    pub fn wants_another_frame(&self) -> bool {
        self.output.as_ref().is_some_and(|output| {
            self.shown_changed() || output.repaint_after < std::time::Duration::MAX
        })
    }

    pub fn drawn(&self) -> Option<&beui::WindowOutput> {
        self.shown_output().filter(|_| self.drew)
    }

    pub fn test_ids(&self) -> std::rc::Rc<beui::accessibility_dump::TestIds> {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_ids()
    }

    fn shown_output(&self) -> Option<&beui::WindowOutput> {
        self.output.as_ref()?.window(self.shown_window?)
    }

    fn shown_changed(&self) -> bool {
        self.shown_output().is_some_and(|shown| shown.changed)
    }

    pub fn document(&self) -> std::cell::Ref<'_, Document> {
        self.plugin
            .document()
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
        Some(report.content)
    }

    pub fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        let mut ticket = None;
        self.call(|editor, connection| {
            ticket = Some(editor.replace_child(connection, old.into_bytes(), new.into_bytes()));
        });
        let ticket = ticket.expect("the call was made");
        self.step(Vec::new());
        let outcome = self
            .completed
            .remove(&ticket.op())
            .expect("the plugin never answered the replacement");
        ticket
            .decode(outcome)
            .expect("the plugin answered the replacement")
    }

    pub fn presence_visible(&mut self, visible: bool) {
        self.call(|editor, connection| editor.presence(connection, visible));
        self.run();
    }

    pub fn resize(&mut self, size: Vec2) {
        let size = block_plugin_api::Size {
            width: size.x,
            height: size.y,
        };
        self.call(|editor, connection| editor.resized(connection, size));
        self.run();
    }

    pub fn set_histories(
        &mut self,
        states: impl IntoIterator<Item = (Uuid, block_editor_beui::BlockHistory)>,
    ) {
        for (block, history) in states {
            self.link
                .offer_history(block, &crate::link::history(history));
        }
    }

    pub fn set_version_status(&mut self, status: block_editor_beui::VersionStatus) {
        self.block_id()
            .expect("a version status is about the editor's own block");
        self.link.offer_version(&status);
    }

    pub fn set_peers(&mut self, block: Option<Uuid>, peers: Vec<PeerPresence>) {
        self.store.set_presence(
            block,
            peers
                .into_iter()
                .map(|peer| (peer.client, peer.kind, peer.value))
                .collect(),
        );
    }

    pub fn receive_from_peer<M: block_editor_beui::be_block::presence::ChannelKind>(
        &mut self,
        from: u64,
        message: &M,
    ) {
        self.block_id()
            .expect("this editor has no block of its own to hear peers on");
        self.store
            .receive_from_peer(None, from, M::ID, message.encode());
    }

    pub fn web_view_event(&mut self, web_view: WebViewId, event: block_editor_beui::WebViewEvent) {
        if let Some((_, watch)) = self.web_views.values_mut().find(|(id, _)| *id == web_view) {
            watch.change(&mut self.connection, |held| {
                let mut events = held.unwrap_or_default();
                block_editor_beui::WebViewEvent::queue(&mut events, event);
                events
            });
            let frames = self.connection.take_outbound();
            self.inbox.push(Inbound::Frames(frames));
        }
    }

    pub fn set_preferences(&mut self, preferences: block_plugin_api::Preferences) {
        let preferences = roundtrip(preferences, "the host's preferences");
        self.inbox.push(Inbound::pushed(move |plugin| {
            plugin.set_preferences(preferences)
        }));
    }

    pub fn set_watched<W: block_plugin_api::Watched>(&mut self, value: &W::Value) {
        let value = block_plugin_api::ops::encode(value);
        for (open, watch) in self.published_watches.values_mut() {
            if *open == W::OPEN {
                watch.set(&mut self.connection, value.clone());
            }
        }
        self.published.insert(W::OPEN, value);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
    }

    pub fn watches<W: block_plugin_api::Watched>(&self) -> bool {
        self.published_watches
            .values()
            .any(|(open, _)| *open == W::OPEN)
    }

    pub fn answer<T: serde::Serialize>(&mut self, reply: Reply<T>, answer: T) {
        reply.send(&mut self.connection, &answer);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
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
        self.child_statuses.change(&mut self.connection, |held| {
            let mut held = held.unwrap_or_default();
            held.extend(statuses);
            held
        });
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
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
            active: false,
            interaction: block_editor_beui::InteractionMode::Preview,
            capabilities: block_editor_beui::EditorCapabilities::default(),
            resize: block_editor_beui::ResizeMode::None,
            error: None,
            menu: Vec::new(),
            creation: None,
            preview: Default::default(),
            dialog: Default::default(),
        });
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size)
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

    pub fn advance(&mut self, by: std::time::Duration) {
        self.clock += by;
        self.run();
    }

    pub fn wait(&mut self, by: std::time::Duration) {
        self.clock += by;
        for message in std::mem::take(&mut self.inbox) {
            self.deliver(message);
        }
        self.hear();
    }

    pub fn settle(&mut self) {
        self.settle_until("its animations to finish", |test| {
            test.output.as_ref().is_none_or(|output| {
                !test.shown_changed() && output.repaint_after > std::time::Duration::ZERO
            })
        });
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
                    (self.shown_changed(), output.repaint_after)
                });
            if changed && eager < EAGER_FRAMES {
                eager += 1;
                continue;
            }
            eager = 0;
            if requested < std::time::Duration::MAX {
                self.clock += requested.saturating_sub(FRAME_INTERVAL);
                continue;
            }
            let left = SETTLE_DEADLINE.saturating_sub(started.elapsed());
            self.wakes.wait_past(seen, left);
        }
        panic!(
            "the editor was still waiting for {what} after {SETTLE_DEADLINE:?}\n{}",
            self.replay()
        );
    }

    pub fn step(&mut self, events: Vec<Input>) {
        self.store.tick(self.clock.as_millis() as u64);
        let mut inbox = std::mem::take(&mut self.inbox);
        self.link.answer(&self.store, &mut self.connection);
        if self.connection.has_outbound() {
            inbox.push(Inbound::Frames(self.connection.take_outbound()));
        }
        let rect = self.rect();
        let band = self.band(rect);
        let intrinsic = self.intrinsic;
        if let Some((view, scale)) = self
            .viewport
            .as_mut()
            .and_then(|viewport| viewport.place(band, rect.min, intrinsic))
        {
            inbox.push(
                self.framed(|editor, connection| editor.view_changed(connection, view, scale)),
            );
        }
        self.clock += FRAME_INTERVAL;
        if !events.is_empty() {
            let now = self.clock.as_micros() as u64;
            let events = events
                .into_iter()
                .map(|input| {
                    let sampled = matches!(
                        input,
                        Input::PointerMoved(_)
                            | Input::PointerButton { .. }
                            | Input::Touch { .. }
                            | Input::Pen { .. }
                    );
                    TimedInput {
                        input,
                        time_micros: sampled.then_some(now),
                    }
                })
                .collect();
            let batch = roundtrip(
                InputBatch {
                    screen: SCREEN,
                    every_sample: false,
                    events,
                },
                "the host's input",
            );
            inbox.push(Inbound::pushed(move |plugin| plugin.input(batch)));
        }
        let now_micros = self.clock.as_micros() as u64;
        inbox.push(Inbound::pushed(move |plugin| {
            plugin.draw_frame(now_micros, true)
        }));
        for message in inbox {
            self.deliver(message);
        }
        self.hear();
        let drawn = self.plugin.draw().and_then(|drawn| {
            let window = drawn
                .windows
                .iter()
                .find(|(placement, _)| placement.screen == SCREEN)
                .map(|(_, window)| *window)?;
            Some((drawn.output, window))
        });
        self.drew = drawn.is_some();
        if let Some((output, window)) = drawn {
            self.output = Some(output);
            self.shown_window = Some(window);
        }
        if let Some(commit) = self.plugin.outbound() {
            self.committed(roundtrip(commit, "the plugin's frame"));
        }
        self.hear();
        let intrinsic = self.intrinsic;
        let band = self.band(rect);
        let drawn = self
            .frame
            .is_none_or(|frame| frame.chrome == FrameChrome::Drawn);
        let placed = self.viewport.as_mut().and_then(|viewport| {
            viewport.settle(&mut self.calls, band, rect.min, intrinsic, drawn);
            viewport.place(band, rect.min, intrinsic)
        });
        if let Some((view, scale)) = placed {
            let framed =
                self.framed(|editor, connection| editor.view_changed(connection, view, scale));
            self.inbox.push(framed);
        }
    }

    fn committed(&mut self, commit: FrameCommit) {
        if let Some(placements) = commit.children.into_iter().last() {
            self.children = placements.children;
            self.occluders = placements.occluders;
        }
        if let Some(report) = commit
            .reports
            .into_iter()
            .find(|report| report.screen == SCREEN)
        {
            self.report = Some(report);
        }
        if let Some(window) = commit
            .windows
            .into_iter()
            .find(|window| window.screen == SCREEN)
            .filter(|_| self.region() == EditorRegion::Frame)
        {
            self.window = window.window;
        }
    }

    fn call(&mut self, call: impl FnOnce(Editor, &mut Connection)) {
        let framed = self.framed(call);
        self.inbox.push(framed);
    }

    fn framed(&mut self, call: impl FnOnce(Editor, &mut Connection)) -> Inbound {
        let editor = self.editor.expect("the test opened its editor");
        call(editor, &mut self.connection);
        Inbound::Frames(self.connection.take_outbound())
    }

    fn hear(&mut self) {
        for frame in self.plugin.take_frames() {
            self.connection.receive(&frame).unwrap_or_else(|error| {
                panic!("the plugin sent a frame that did not decode: {error}")
            });
        }
        while let Some(event) = self.connection.next_event() {
            let called = match event {
                Incoming::Started(started) if ShellCall::accepts(&started) => {
                    let connection = &mut self.connection;
                    match ShellCall::accept(connection, started) {
                        Some(ShellCall::ChildStatusesWatchNew { resource, .. }) => {
                            self.child_statuses.opened(resource.op())
                        }
                        Some(ShellCall::ChildStatusesWatchNext { reply, .. }) => {
                            self.child_statuses.next(connection, reply)
                        }
                        Some(ShellCall::BlockTypesWatchNew { resource }) => {
                            self.block_types.opened(resource.op())
                        }
                        Some(ShellCall::BlockTypesWatchNext { reply, .. }) => {
                            self.block_types.next(connection, reply)
                        }
                        Some(ShellCall::WebViewNew {
                            resource, id, url, ..
                        }) => {
                            let mut watch = block_plugin_api::Watch::default();
                            watch.opened(resource.op());
                            self.web_views.insert(resource.op(), (id, watch));
                            self.web_view_commands.push((id, WebViewCommand::Open(url)));
                        }
                        Some(ShellCall::WebViewNextEvents { this, reply }) => {
                            match self.web_views.get_mut(&this.op()) {
                                Some((_, watch)) => watch.next(connection, reply),
                                None => {
                                    reply.fail(connection, block_plugin_api::ops::OpError::Canceled)
                                }
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                Incoming::Started(started) if DebugCall::accepts(&started) => {
                    let connection = &mut self.connection;
                    match DebugCall::accept(connection, started) {
                        Some(DebugCall::DebugTemplatesWatchNew { resource }) => {
                            self.debug_templates.opened(resource.op())
                        }
                        Some(DebugCall::DebugTemplatesWatchNext { reply, .. }) => {
                            self.debug_templates.next(connection, reply)
                        }
                        _ => {}
                    }
                    continue;
                }
                Incoming::Started(started) if published(started.method).is_some() => {
                    let connection = &mut self.connection;
                    let watching = published(started.method).unwrap_or_else(|| unreachable!());
                    if watching.open == started.method {
                        let mut watch = block_plugin_api::Watch::default();
                        watch.opened(started.op);
                        if let Some(value) = self.published.get(&watching.open) {
                            watch.set(connection, value.clone());
                        }
                        self.published_watches
                            .insert(started.op, (watching.open, watch));
                        continue;
                    }
                    match started
                        .parent
                        .and_then(|parent| self.published_watches.get_mut(&parent))
                    {
                        Some((_, watch)) => watch.next_encoded(connection, started.op),
                        None => connection.complete(started.op, Outcome::Err(OpError::Canceled)),
                    }
                    continue;
                }
                Incoming::Started(started) if BlocksCall::accepts(&started) => {
                    if let Some(call) = BlocksCall::accept(&mut self.connection, started) {
                        self.link.call(call, &self.store, &mut self.connection);
                    }
                    continue;
                }
                Incoming::Canceled { op } if self.link.holds(op) => {
                    self.link.ended(op, &self.store, &mut self.connection);
                    continue;
                }
                Incoming::Called(called) => called,
                Incoming::Completed { op, outcome } => {
                    self.completed.insert(op, outcome);
                    continue;
                }
                Incoming::Started(started) => {
                    let op = started.op;
                    if Call::accepts(&started)
                        && let Some(call) = Call::accept(&mut self.connection, started)
                    {
                        self.requests.push((Some(op), call));
                    }
                    continue;
                }
                Incoming::Canceled { op } => {
                    let connection = &mut self.connection;
                    if self
                        .published_watches
                        .values_mut()
                        .any(|(_, watch)| watch.withdraw(connection, op))
                    {
                        continue;
                    }
                    self.published_watches.remove(&op);
                    if let Some((id, _)) = self.web_views.remove(&op) {
                        self.web_view_commands.push((id, WebViewCommand::Close));
                    }
                    self.requests.retain(|(asked, _)| *asked != Some(op));
                    self.connection
                        .complete(op, Outcome::Err(OpError::Canceled));
                    continue;
                }
                Incoming::Yielded { .. } | Incoming::Credited { .. } => continue,
            };
            if let Some(call) = Call::accept_direct(&called) {
                self.requests.push((None, call));
                continue;
            }
            if let Some(call) = BlocksCall::accept_direct(&called) {
                self.link.call(call, &self.store, &mut self.connection);
                continue;
            }
            if DebugCall::accept_direct(&called).is_some() {
                continue;
            }
            if called
                .name
                .split_once('#')
                .is_some_and(|(interface, _)| block_plugin_api::is_published(interface))
            {
                self.extension_calls.push(called);
                continue;
            }
            let Some(call) = ShellCall::accept_direct(&called) else {
                panic!(
                    "the plugin called {}, which the test does not answer",
                    called.name
                );
            };
            let shown = |this: &block_plugin_api::embed::shell::WebView| {
                self.web_views.get(&this.op()).map(|(id, _)| *id)
            };
            let commanded = match &call {
                ShellCall::WebViewLoad { this, url } => {
                    shown(this).map(|id| (id, WebViewCommand::Load(url.clone())))
                }
                ShellCall::WebViewReload { this } => {
                    shown(this).map(|id| (id, WebViewCommand::Reload))
                }
                ShellCall::WebViewFocusApp { this } => {
                    shown(this).map(|id| (id, WebViewCommand::FocusApp))
                }
                _ => None,
            };
            if let Some(commanded) = commanded {
                self.web_view_commands.push(commanded);
                continue;
            }
            match &call {
                ShellCall::Menu { items, .. } => {
                    self.menu = MenuEntry::nest(items.clone()).unwrap_or_else(|error| {
                        panic!("the plugin sent a menu it may not: {error}")
                    });
                }
                ShellCall::LeaveFrame { .. } => self.exited = true,
                ShellCall::ArtifactEdited { data, .. } => self.draft.clone_from(data),
                ShellCall::IntrinsicSize { size, .. } => {
                    self.intrinsic = size.map(|size| Vec2::new(size.width, size.height));
                }
                _ => {}
            }
            self.calls.push(call);
        }
        self.link.answer(&self.store, &mut self.connection);
        if self.connection.has_outbound() {
            let frames = self.connection.take_outbound();
            self.inbox.push(Inbound::Frames(frames));
        }
    }

    fn take_calls<T>(&mut self, pick: impl Fn(&ShellCall) -> Option<T>) -> Vec<T> {
        let mut taken = Vec::new();
        self.calls.retain(|call| match pick(call) {
            Some(value) => {
                taken.push(value);
                false
            }
            None => true,
        });
        taken
    }

    pub fn clear_calls(&mut self) {
        self.calls.clear();
        self.extension_calls.clear();
    }

    pub fn take_shell_commands(&mut self) -> Vec<(Uuid, block_plugin_api::ShellCommand)> {
        self.take_calls(|call| match call {
            ShellCall::Act { block, command, .. } => Some((Uuid::from_bytes(*block), *command)),
            _ => None,
        })
    }

    pub fn take_copied_text(&mut self) -> Vec<String> {
        self.take_calls(|call| match call {
            ShellCall::CopyText { text, .. } => Some(text.clone()),
            _ => None,
        })
    }

    pub fn take_copied_images(&mut self) -> Vec<Vec<u8>> {
        self.take_calls(|call| match call {
            ShellCall::CopyImage { png, .. } => Some(png.clone()),
            _ => None,
        })
    }

    pub fn request_pick(
        &mut self,
        pick: u64,
        filter: block_editor_beui::BlockFilter,
        parent: block_plugin_api::BlockLocation,
        within: Option<Uuid>,
    ) {
        self.ask_pick(pick, filter, parent, within, false);
    }

    pub fn request_touch_pick(
        &mut self,
        pick: u64,
        filter: block_editor_beui::BlockFilter,
        parent: block_plugin_api::BlockLocation,
        within: Option<Uuid>,
    ) {
        self.ask_pick(pick, filter, parent, within, true);
    }

    fn ask_pick(
        &mut self,
        pick: u64,
        filter: block_editor_beui::BlockFilter,
        parent: block_plugin_api::BlockLocation,
        within: Option<Uuid>,
        touch: bool,
    ) {
        let request = block_plugin_api::shell_types::PickRequest {
            pick,
            filter,
            parent,
            within: within.map(Uuid::into_bytes),
            touch,
        };
        let mut asked = None;
        self.call(|editor, connection| asked = Some(editor.pick(connection, request)));
        self.asked_picks.extend(asked.map(|ticket| (pick, ticket)));
    }

    pub fn show_dialog(&mut self, block: Uuid, dialog: block_plugin_api::ShellDialog) {
        self.call(|editor, connection| editor.show_dialog(connection, block.into_bytes(), dialog));
    }

    pub fn take_renames(&mut self) -> Vec<(Uuid, Option<String>)> {
        self.take_sent_calls(|sent| match sent {
            Sent::Define(block, key, value) if *key == block_editor_beui::be_block::NAME.id => {
                Some((
                    *block,
                    value
                        .as_ref()
                        .and_then(|value| block_editor_beui::be_block::be_model::from_cbor(value)),
                ))
            }
            _ => None,
        })
    }

    pub fn take_definitions(&mut self, key: Uuid) -> Vec<(Uuid, Option<Vec<u8>>)> {
        self.take_sent_calls(|sent| match sent {
            Sent::Define(block, defined, value) if *defined == key => Some((*block, value.clone())),
            _ => None,
        })
    }

    fn take_sent_calls<T>(&mut self, pick: impl Fn(&Sent) -> Option<T>) -> Vec<T> {
        let mut taken = Vec::new();
        self.link.sent.retain(|sent| match pick(sent) {
            Some(value) => {
                taken.push(value);
                false
            }
            None => true,
        });
        taken
    }

    pub fn take_access_changes(&mut self) -> Vec<(Uuid, Uuid, block_plugin_api::AccessLevel)> {
        self.take_requests_where(|call| match call {
            Call::SetAccess {
                block,
                account,
                access,
            } => Ok((Uuid::from_bytes(block), Uuid::from_bytes(account), access)),
            call => Err(call),
        })
    }

    pub fn withdraw_pick(&mut self, pick: u64) {
        let asked = self
            .asked_picks
            .iter()
            .find(|(asked, _)| *asked == pick)
            .map(|(_, ticket)| ticket.op())
            .expect("the test asked for this pick");
        self.connection.cancel(asked);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
        self.run();
    }

    pub fn take_pick_answers(&mut self) -> Vec<(u64, block_editor_beui::BlockPick)> {
        let mut answers = Vec::new();
        let completed = &mut self.completed;
        self.asked_picks
            .retain(|(pick, ticket)| match completed.remove(&ticket.op()) {
                Some(outcome) => {
                    answers.push((*pick, ticket.decode(outcome).unwrap_or(Ok(None))));
                    false
                }
                None => true,
            });
        answers
    }

    pub fn take_child_commits(
        &mut self,
    ) -> Vec<(ChildId, block_plugin_api::BlockLocation, Option<String>)> {
        self.take_calls(|call| match call {
            ShellCall::CommitChild {
                child,
                parent,
                name,
                ..
            } => Some((*child, *parent, name.clone())),
            _ => None,
        })
    }

    pub fn take_audio_commands(&mut self) -> Vec<block_plugin_api::AudioCommand> {
        self.take_calls(|call| match call {
            ShellCall::PlayAudio { command, .. } => Some(command.clone()),
            _ => None,
        })
    }

    pub fn take_view_changes(&mut self) -> Vec<ViewChange> {
        self.take_calls(|call| match call {
            ShellCall::ChangeView { change, .. } => Some(*change),
            _ => None,
        })
    }

    pub fn take_child_menu_picks(&mut self) -> Vec<(ChildId, String)> {
        self.take_calls(|call| match call {
            ShellCall::ChildMenuPick { child, id, .. } => Some((*child, id.clone())),
            _ => None,
        })
    }

    pub fn take_bar_actions(&mut self) -> Vec<BarAction> {
        self.take_calls(|call| match call {
            ShellCall::RunBarAction { action, .. } => Some(*action),
            _ => None,
        })
    }

    pub fn has_requests(&self) -> bool {
        !self.requests.is_empty()
    }

    pub fn take_requests(&mut self) -> Vec<Call> {
        std::mem::take(&mut self.requests)
            .into_iter()
            .map(|(_, call)| call)
            .collect()
    }

    pub fn take_requests_where<T>(&mut self, pick: impl Fn(Call) -> Result<T, Call>) -> Vec<T> {
        let mut picked = Vec::new();
        for (op, call) in std::mem::take(&mut self.requests) {
            match pick(call) {
                Ok(value) => picked.push(value),
                Err(call) => self.requests.push((op, call)),
            }
        }
        picked
    }

    pub fn take_opens(&mut self) -> Vec<block_editor_beui::OpenRequest> {
        self.take_sent_calls(|sent| match sent {
            Sent::Open(open) => Some(block_editor_beui::OpenRequest {
                block_id: Uuid::from_bytes(open.block_id),
                block_type: Uuid::from_bytes(open.block_type),
                via: open.via.map(Uuid::from_bytes),
                editor: open.editor.map(Uuid::from_bytes),
            }),
            _ => None,
        })
    }

    pub fn take_block_commands(&mut self) -> Vec<(Uuid, block_editor_beui::BlockCommand)> {
        self.take_sent_calls(|sent| match sent {
            Sent::Command(block, command) => Some((*block, *command)),
            _ => None,
        })
    }

    pub fn host_calls<C>(
        &self,
        accept: impl Fn(&block_plugin_api::ops::Called) -> Option<C>,
    ) -> Vec<C> {
        self.extension_calls.iter().filter_map(accept).collect()
    }

    pub fn take_host_calls<C>(
        &mut self,
        accept: impl Fn(&block_plugin_api::ops::Called) -> Option<C>,
    ) -> Vec<C> {
        let mut taken = Vec::new();
        self.extension_calls.retain(|called| match accept(called) {
            Some(call) => {
                taken.push(call);
                false
            }
            None => true,
        });
        taken
    }

    pub fn take_version_commands(&mut self) -> Vec<(Uuid, block_editor_beui::VersionCommand)> {
        self.take_sent_calls(|sent| match sent {
            Sent::Version(block, command) => Some((*block, command.clone())),
            _ => None,
        })
    }

    fn take_frames<T>(&mut self, pick: impl Fn(Uuid, FromEditor) -> Option<T>) -> Vec<T> {
        self.take_sent_calls(|sent| match sent {
            Sent::Frame(block, frame) => pick(*block, frame.clone()),
            _ => None,
        })
    }

    pub fn take_shown_presence(&mut self) -> Vec<ShownPresence> {
        self.take_frames(|block, frame| match frame {
            FromEditor::ShowPresence { kind, value } => Some(ShownPresence {
                block: Some(block),
                kind,
                value,
            }),
            _ => None,
        })
    }

    pub fn take_sent_to_peers<M: block_editor_beui::be_block::presence::ChannelKind>(
        &mut self,
    ) -> Vec<(Option<u64>, M)> {
        self.take_frames(|_, frame| match frame {
            FromEditor::SendToPeers {
                to,
                channel,
                payload,
            } if channel == M::ID => M::decode(&payload).map(|message| (to, message)),
            _ => None,
        })
    }

    pub fn take_web_view_commands(&mut self) -> Vec<(WebViewId, WebViewCommand)> {
        std::mem::take(&mut self.web_view_commands)
    }

    pub fn take_drag_accepted(&mut self) -> Option<bool> {
        self.take_calls(|call| match call {
            ShellCall::DragAccepted { accepted, .. } => Some(*accepted),
            _ => None,
        })
        .pop()
    }

    pub fn pointer_locked(&self) -> bool {
        self.window.pointer_locked
    }

    pub fn pointer_motion(&mut self, delta: Vec2) {
        self.push(Input::PointerMotion(delta));
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

    fn push(&mut self, event: Input) {
        match self.frames.last_mut() {
            Some(frame) => frame.push(event),
            None => self.frames.push(vec![event]),
        }
    }

    fn perform(&mut self, frames: Frames) {
        for (index, frame) in frames.into_iter().enumerate() {
            if index > 0 {
                self.next_frame();
            }
            for event in frame {
                self.push(event);
            }
        }
    }

    pub fn next_frame(&mut self) {
        if self.frames.last().is_some_and(|frame| !frame.is_empty()) {
            self.frames.push(Vec::new());
        }
    }

    pub fn back(&mut self) {
        let frames = self.gestures.back();
        self.perform(frames);
    }

    pub fn handles_back(&self) -> bool {
        self.window.handles_back
    }

    pub fn wants_keyboard(&self) -> bool {
        self.window.wants_keyboard
    }

    pub fn window(&self) -> &block_plugin_api::WindowRequests {
        &self.window
    }

    pub fn intercepted_keys(&self) -> Vec<KeyChord> {
        self.window.intercepted_keys.clone()
    }

    pub fn app_key(&mut self, modifiers: Modifiers, key: Key) -> bool {
        self.hold_app_key(modifiers, key, 0)
    }

    pub fn hold_app_key(&mut self, modifiers: Modifiers, key: Key, repeats: usize) -> bool {
        if !self
            .intercepted_keys()
            .contains(&KeyChord::new(key, modifiers))
        {
            return false;
        }
        self.hold_modifiers(modifiers);
        let presses = std::iter::once((true, false))
            .chain(std::iter::repeat_n((true, true), repeats))
            .chain(std::iter::once((false, false)));
        for (pressed, repeat) in presses {
            self.push(Input::InterceptedKey {
                key,
                pressed,
                repeat,
            });
        }
        true
    }

    pub fn app_tap(&mut self, key: Key) -> bool {
        if !self.intercepted_keys().contains(&KeyChord::tap(key)) {
            return false;
        }
        self.push(Input::InterceptedTap(key));
        true
    }

    pub fn hold_modifiers(&mut self, modifiers: Modifiers) {
        let frames = self.gestures.hold(modifiers);
        self.perform(frames);
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
        collect_text(&self.document(), node, &mut collected);
        collected.join(" ")
    }

    pub fn accessible_label(&self, test_id: &str) -> Option<String> {
        let document = self.document();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
        document
            .accessibility
            .get(&node)?
            .label()
            .map(str::to_owned)
    }

    pub fn rect_of(&self, test_id: &str) -> Rect {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_id_rect(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}\n{}", self.replay()))
    }

    pub fn rects_of(&self, test_id: &str) -> Vec<Rect> {
        self.test_ids()
            .iter()
            .find(|(named, _)| *named == test_id)
            .map(|(_, shown)| shown.iter().map(|at| at.rect).collect())
            .unwrap_or_default()
    }

    pub fn point_of(&self, test_id: &str) -> Pos2 {
        let rect = self.rect_of(test_id);
        let shown = rect.intersect(self.rect());
        match shown.is_positive() {
            true => shown.center(),
            false => rect.center(),
        }
    }

    pub fn reveal(&mut self, test_id: &str) {
        if self.rect().contains_rect(self.rect_of(test_id)) {
            return;
        }
        {
            let mut document = self
                .plugin
                .document_mut()
                .expect("the editor has not built its view yet");
            let node = document
                .find_test_id(test_id)
                .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
            document.reveal_node(node);
        }
        self.run();
    }

    pub fn secondary_drag(&mut self, from: Pos2, to: Pos2) {
        self.steps.push(format!(
            "drive app down {} right; drive app up {} right",
            Target::point(from).word(1.0),
            Target::point(to).word(1.0)
        ));
        let mut frames = self.gestures.down(from, PointerButton::Secondary);
        frames.extend(self.gestures.up(Some(to), PointerButton::Secondary));
        self.perform(frames);
    }

    pub fn pen(&mut self, phase: PenPhase, pos: Pos2, pressure: f32) {
        let frames = self.gestures.pen_sample(
            phase,
            pos,
            Pen {
                pressure,
                ..Pen::default()
            },
        );
        self.perform(frames);
    }

    pub fn eraser(&mut self, phase: PenPhase, pos: Pos2) {
        let eraser = Pen {
            pressure: 1.0,
            tilt: Vec2::ZERO,
            eraser: true,
        };
        let frames = self.gestures.pen_sample(phase, pos, eraser);
        self.perform(frames);
    }

    pub fn pinch(&mut self, pos: Pos2, factor: f32) {
        let anchor = Pos2::ZERO + (pos - self.rect().min);
        self.calls.push(ShellCall::ChangeView {
            instance: INSTANCE,
            change: ViewChange::Zoom {
                factor,
                anchor: Some(anchor),
            },
        });
        self.run();
    }

    pub fn predict(&mut self, pos: Pos2) {
        self.push(Input::Predicted(pos));
    }

    pub fn hover(&mut self, target: impl Into<Target>) {
        let at = self.locate("move", &[target.into()], &[]);
        let frames = self.gestures.hover(at[0]);
        self.perform(frames);
    }

    pub fn click(&mut self, target: impl Into<Target>) {
        let at = self.locate("click", &[target.into()], &[]);
        let frames = self.gestures.click(at[0], PointerButton::Primary, 1);
        self.perform(frames);
    }

    pub fn click_with(&mut self, target: impl Into<Target>, button: PointerButton) {
        let at = self.locate("click", &[target.into()], &[button_name(button)]);
        let frames = self.gestures.click(at[0], button, 1);
        self.perform(frames);
    }

    pub fn double_click(&mut self, target: impl Into<Target>) {
        let at = self.locate("click", &[target.into()], &["double".to_owned()]);
        let frames = self.gestures.click(at[0], PointerButton::Primary, 2);
        self.perform(frames);
    }

    pub fn tap(&mut self, target: impl Into<Target>) {
        let at = self.locate("tap", &[target.into()], &[]);
        let frames = self.gestures.tap(at[0], 1);
        self.perform(frames);
    }

    pub fn down(&mut self, target: impl Into<Target>, button: PointerButton) {
        let at = self.locate("down", &[target.into()], &[button_name(button)]);
        let frames = self.gestures.down(at[0], button);
        self.perform(frames);
    }

    pub fn up(&mut self, target: impl Into<Target>, button: PointerButton) {
        let at = self.locate("up", &[target.into()], &[button_name(button)]);
        let frames = self.gestures.up(Some(at[0]), button);
        self.perform(frames);
    }

    pub fn drag(&mut self, from: impl Into<Target>, to: impl Into<Target>) {
        let at = self.locate("drag", &[from.into(), to.into()], &[]);
        let frames = self.gestures.drag(at[0], at[1], 2);
        self.perform(frames);
    }

    pub fn drag_along(&mut self, points: &[Pos2]) {
        let (Some(&from), Some(&to)) = (points.first(), points.last()) else {
            return;
        };
        self.down(from, PointerButton::Primary);
        for &at in &points[1..] {
            self.next_frame();
            self.hover(at);
        }
        self.next_frame();
        self.up(to, PointerButton::Primary);
    }

    pub fn scroll(&mut self, target: impl Into<Target>, by: Vec2) {
        let rest = [by.y.to_string(), by.x.to_string()];
        let at = self.locate("scroll", &[target.into()], &rest);
        let frames = self.gestures.scroll(at[0], by);
        self.perform(frames);
    }

    pub fn fingers<T: Into<Target> + Clone>(&mut self, phase: TouchPhase, fingers: &[(u64, T)]) {
        let targets: Vec<Target> = fingers
            .iter()
            .map(|(_, target)| target.clone().into())
            .collect();
        let named: Vec<String> = fingers
            .iter()
            .zip(&targets)
            .map(|((finger, _), target)| format!("{finger}={}", target.word(1.0)))
            .collect();
        let phase_name = match phase {
            TouchPhase::Start => "down",
            TouchPhase::Move => "move",
            TouchPhase::End => "up",
            TouchPhase::Cancel => "cancel",
        };
        self.steps
            .push(format!("drive app touch {phase_name} {}", named.join(" ")));
        let at: Vec<(u64, Option<Pos2>)> = fingers
            .iter()
            .zip(targets)
            .map(|((finger, _), target)| (*finger, Some(self.point(target))))
            .collect();
        let frames = self
            .gestures
            .touch(phase, &at)
            .unwrap_or_else(|error| panic!("{error}\n{}", self.replay()));
        self.perform(frames);
    }

    pub fn type_text(&mut self, text: &str) {
        self.steps.push(format!("drive app type {text:?}"));
        let frames = self.gestures.type_text(text);
        self.perform(frames);
    }

    pub fn key(&mut self, key: Key, modifiers: Modifiers) {
        let chord = beui_core::input::typing::chord_name(key, modifiers)
            .unwrap_or_else(|| format!("{modifiers:?}+{key:?}"));
        self.steps.push(format!("drive app key {chord}"));
        let frames = self.gestures.chord(key, modifiers);
        self.perform(frames);
    }

    pub fn key_down(&mut self, key: Key) {
        self.held_key(key, true);
    }

    pub fn key_up(&mut self, key: Key) {
        self.held_key(key, false);
    }

    fn held_key(&mut self, key: Key, pressed: bool) {
        let command = if pressed { "keydown" } else { "keyup" };
        let name = key.name().unwrap_or_else(|| format!("{key:?}"));
        self.steps.push(format!("drive app {command} {name}"));
        let frames = self
            .gestures
            .key(key, pressed)
            .unwrap_or_else(|error| panic!("{error}\n{}", self.replay()));
        self.perform(frames);
    }

    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    fn locate(&mut self, command: &str, targets: &[Target], rest: &[String]) -> Vec<Pos2> {
        let mut step = format!("drive app {command}");
        for word in targets
            .iter()
            .map(|target| target.word(1.0))
            .chain(rest.iter().cloned())
        {
            step.push(' ');
            step.push_str(&word);
        }
        self.steps.push(step);
        targets
            .iter()
            .map(|target| self.point(target.clone()))
            .collect()
    }

    fn point(&mut self, target: Target) -> Pos2 {
        match target {
            Target::Id(id, offset) => {
                self.reveal(&id);
                match offset {
                    Some(offset) => self.rect_of(&id).min + offset,
                    None => self.point_of(&id),
                }
            }
            Target::Point(at) => at,
            Target::Text(text) => panic!(
                "a plugin test finds a target by its test id or a point, not by text: {text:?}\n{}",
                self.replay()
            ),
        }
    }

    fn replay(&self) -> String {
        let mut replay = String::from("the steps so far, as drive commands:\n");
        for step in &self.steps {
            replay.push_str("  ");
            replay.push_str(step);
            replay.push('\n');
        }
        replay
    }

    pub fn record(&mut self) {
        let frame = self.painted();
        let found = overflow::overflows(&self.document());
        self.overflows.extend(found);
        match &mut self.recording {
            Some(recording) => recording.append(frame),
            None => self.recording = Some(frame),
        }
    }

    pub fn snapshot(&mut self, name: &str) {
        let painting = match self.recording.take() {
            Some(recording) => recording,
            None => {
                let found = overflow::overflows(&self.document());
                self.overflows.extend(found);
                self.painted()
            }
        };
        snapshot::assert_snapshot(name, &painting);
        overflow::assert_fits(name, std::mem::take(&mut self.overflows));
    }

    fn painted(&mut self) -> snapshot::Snapshot {
        let behind = beui::styled::Theme::of(self.document().preferences().theme).background;
        let output = self
            .output
            .as_ref()
            .expect("the editor has not drawn a frame yet");
        let shown = self
            .shown_output()
            .expect("the editor has not drawn its window yet");
        capture::capture(shown, self.size, output.pixels_per_point(), behind)
            .expect("the painting could not be rendered")
    }
}

fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned>(value: T, what: &str) -> T {
    block_plugin_api::ops::decode(&block_plugin_api::ops::encode(&value))
        .unwrap_or_else(|| panic!("{what} could not be read back"))
}

fn collect_text(document: &Document, node: beui::NodeId, collected: &mut Vec<String>) {
    if document.node_kind(node) == "text" {
        let detail = document.node_detail(node).unwrap_or_default();
        let text = detail.trim_matches('"');
        collected.extend(
            document
                .text_icons(node)
                .into_iter()
                .map(|glyph| glyph.as_str().to_owned()),
        );
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
    moved: bool,
    world: Option<Vec2>,
    fitted: Option<FitArea>,
    sent: Option<(Rect, f32)>,
}

impl Viewport {
    fn new() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            moved: false,
            world: None,
            fitted: None,
            sent: None,
        }
    }

    fn show(&mut self, view: Rect, scale: f32, region: Rect, origin: Pos2) {
        let scale = scale.max(f32::EPSILON);
        self.zoom = scale;
        self.pan = view.center() + origin.to_vec2() - region.center();
        self.world = Some(view.size() / scale);
        self.moved = true;
        self.sent = Some((view, scale));
    }

    fn content(&self, region: Rect, intrinsic: Option<Vec2>) -> Vec2 {
        self.world.unwrap_or_else(|| {
            intrinsic
                .unwrap_or(Vec2::ZERO)
                .max(region.size())
                .max(Vec2::new(1.0, 1.0))
        })
    }

    fn place(
        &mut self,
        region: Rect,
        origin: Pos2,
        intrinsic: Option<Vec2>,
    ) -> Option<(Rect, f32)> {
        let size = self.content(region, intrinsic) * self.zoom;
        let center = region.center() + self.pan;
        let view = Rect::from_min_size(center - size * 0.5, size).translate(-origin.to_vec2());
        let sent = (view, self.zoom);
        if self.sent == Some(sent) {
            return None;
        }
        self.sent = Some(sent);
        Some(sent)
    }

    fn settle(
        &mut self,
        sent: &mut Vec<ShellCall>,
        region: Rect,
        origin: Pos2,
        intrinsic: Option<Vec2>,
        drawn: bool,
    ) {
        let mut changes = Vec::new();
        sent.retain(|call| match call {
            ShellCall::ChangeView { change, .. } => {
                changes.push(*change);
                false
            }
            _ => true,
        });
        for change in changes {
            match change {
                ViewChange::KeepFitted(area) => self.fitted = area,
                ViewChange::Fit => self.moved = false,
                ViewChange::Zoom { factor, anchor } => self.apply(
                    ViewChange::Zoom {
                        factor,
                        anchor: anchor.map(|anchor| anchor + origin.to_vec2()),
                    },
                    region,
                ),
                change => self.apply(change, region),
            }
        }
        if let Some(area) = self.fitted.filter(|_| drawn && !self.moved) {
            let content = self.content(region, intrinsic);
            let (scale, centre) = area.fit(content, region.size());
            self.zoom = scale.clamp(MINIMUM_ZOOM, MAXIMUM_ZOOM);
            self.pan = (content * 0.5 - centre.to_vec2()) * self.zoom;
        }
    }

    fn apply(&mut self, change: ViewChange, region: Rect) {
        match change {
            ViewChange::Pan(by) => self.pan += by,
            ViewChange::Zoom { factor, anchor } => {
                let anchor = anchor.unwrap_or(region.center());
                self.zoom_at(factor, anchor, region);
            }
            ViewChange::KeepFitted(_) | ViewChange::Fit | ViewChange::PanDrag => return,
        }
        self.moved = true;
    }

    fn zoom_at(&mut self, factor: f32, anchor: Pos2, region: Rect) {
        let zoom = (self.zoom * factor).clamp(MINIMUM_ZOOM, MAXIMUM_ZOOM);
        let anchor = anchor - region.center();
        self.pan = anchor - (anchor - self.pan) * (zoom / self.zoom);
        self.zoom = zoom;
    }
}

fn published(method: block_plugin_api::ops::MethodId) -> Option<block_plugin_api::ops::Watching> {
    block_plugin_api::embed::WATCHES
        .iter()
        .find(|watching| {
            (watching.open == method || watching.next == method)
                && block_plugin_api::is_published(watching.interface)
        })
        .copied()
}
