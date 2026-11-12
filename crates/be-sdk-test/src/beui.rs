use be_sdk_api::app::{ChildBarPress, DialogRequest, EditorEvent, MenuChoice, ViewFrame};
use be_sdk_api::embed::debug::debug::Call as DebugCall;
use be_sdk_api::embed::{self, exports::app::Editor, shell::Call as ShellCall};
use be_sdk_api::ops::{Connection, Incoming, Side};
use be_sdk_api::shell_types::{
    BlockAct, ChildCreation, ChildMenuChoice, ClipOffer, MenuSet, ShellMessage,
};
use be_sdk_api::{
    BarAction, BlockTypeDescriptor, Catalog, ChildId, FitArea, FrameChrome, FrameReport,
    HelloAccepted, InputBatch, LayoutCommit, MenuCommand, MenuEntry, OpenArtifact, OpenBlock,
    OpenCreation, ScreenId, ScreenRequest, SurfaceFormat, SurfaceSpec, TimedInput, TopBar, View,
    ViewId, ViewportMetrics,
};
use be_sdk_api::{Opening, OpeningKind};
use be_sdk_api::{
    embed::host_services::Call,
    ops::{OpError, OpId, Outcome, Reply},
};
use be_sdk_beui::FromEditor;
use be_sdk_beui::be_block::{LiveEdit, Merge};
use be_sdk_beui::headless::{Adopted, HeadlessApp};
use be_sdk_beui::{
    Artifacts, BeuiRoot, ChildMode, ChildPlacement, ChildStatus, Creation, EditorHost,
    EditorInstanceId, Occluder, OwnedEditor, PeerPresence, SeededContent, ShownPresence,
    ViewChange, WebViewCommand, WebViewId,
};
use beui::native::Document;
use beui::{
    Input, Key, KeyChord, Modifiers, Pen, PenPhase, PointerButton, Pos2, Rect, TouchPhase, Vec2,
};
use beui_host::native::testing::{Target, button_name};
use std::marker::PhantomData;

type Delivered = Box<dyn FnOnce(&mut HeadlessApp) -> Result<(), String>>;

enum Inbound {
    Pushed(Delivered),
    Frames(Vec<Vec<u8>>),
}

impl Inbound {
    fn pushed(deliver: impl FnOnce(&mut HeadlessApp) -> Result<(), String> + 'static) -> Self {
        Self::Pushed(Box::new(deliver))
    }
}

pub type HostCall = Call;

use std::sync::{Arc, Condvar, Mutex, PoisonError};
use uuid::Uuid;

use crate::ContentStore;
use crate::link::Sent;
use beui_host::native::drive;
use beui_host::native::drive::gestures::{Frames, Gestures};
use beui_host::native::testing::{FRAME_INTERVAL, assert_fits};
use beui_paint_snapshot::Snapshot;

mod desk;

const SIZE: Vec2 = Vec2::new(800.0, 600.0);
const EAGER_FRAMES: usize = 8;
const SETTLE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
const HOST_ROUNDS: usize = 8;
const MINIMUM_ZOOM: f32 = 1.0 / 64.0;
const MAXIMUM_ZOOM: f32 = 32.0;
const INSTANCE: EditorInstanceId = EditorInstanceId(1);

thread_local! {
    static PICKER: std::cell::RefCell<Option<be_sdk_beui::PickerRequest>> =
        const { std::cell::RefCell::new(None) };
    static SHARED_STORE: std::cell::RefCell<Option<ContentStore>> =
        const { std::cell::RefCell::new(None) };
}

const ADOPTED_EDITOR: [u8; 16] = [0; 16];
const SCREEN: ScreenId = ScreenId(1);

/// Runs an editor headless through the app's own session (`be_sdk::headless`), acting as
/// its host: there is no server or block store, and every call goes over a be-wasm-ops
/// connection encoded as on the wire, so a call the app refuses fails the test.
///
/// Gestures (`click`, `drag`, `text`, `tap`, ...) are drive's and only queue input: nothing
/// they cause shows until the next `run()`, so call it after each one. A click reveals an
/// out-of-view target first and panics when no node has the test id. Whatever the app sends
/// the host is kept for the `take_*` methods; `clear_calls` drops it. Calls such as
/// `set_view` queue what be-host would send; setting the same state on the `EditorHost`
/// directly skips the protocol and is overwritten by the session on the next frame.
pub struct BeuiTest<A: BeuiRoot> {
    app: HeadlessApp,
    kind: Kind,
    host: EditorHost,
    store: ContentStore,
    size: Vec2,
    scale_factor: f32,
    raster: Option<f32>,
    frame: Option<be_sdk_api::FrameSpec>,
    gestures: Gestures,
    steps: Vec<String>,
    frames: Vec<Vec<Input>>,
    inbox: Vec<Inbound>,
    answered: Vec<Vec<u8>>,
    acknowledged: std::collections::HashSet<OpId>,
    connection: Connection,
    link: crate::link::Link,
    editor: Option<Editor>,
    calls: Vec<ShellMessage>,
    extension_calls: Vec<be_sdk_api::ops::Started>,
    web_views: std::collections::HashMap<
        be_sdk_api::ops::OpId,
        (WebViewId, be_sdk_api::Watch<Vec<be_sdk_beui::WebViewEvent>>),
    >,
    web_view_commands: Vec<(WebViewId, WebViewCommand)>,
    published: std::collections::HashMap<be_sdk_api::ops::MethodId, Vec<u8>>,
    published_watches: std::collections::HashMap<
        be_sdk_api::ops::OpId,
        (
            be_sdk_api::ops::MethodId,
            be_sdk_api::Watch<be_sdk_api::Encoded>,
        ),
    >,
    child_statuses: be_sdk_api::Watch<Vec<ChildStatus>>,
    block_types: be_sdk_api::Watch<Catalog>,
    shown_catalog: Option<Catalog>,
    debug_templates: be_sdk_api::Watch<Vec<be_sdk_api::DebugTemplate>>,
    completed: std::collections::HashMap<be_sdk_api::ops::OpId, be_sdk_api::ops::Outcome>,
    menu: Vec<MenuEntry>,
    docked: bool,
    docks: Vec<be_sdk_api::DockMessage>,
    output: Option<beui::native::FrameOutput>,
    shown_window: Option<beui::WindowId>,
    drew: bool,
    recording: Option<Snapshot>,
    overflows: Vec<String>,
    viewport: Option<Viewport>,
    children: Vec<ChildPlacement>,
    views_children: Vec<(ViewId, Vec<ChildPlacement>)>,
    occluders: Vec<Occluder>,
    report: Option<FrameReport>,
    actions: Vec<be_sdk_api::DescribedAction>,
    window: be_sdk_api::WindowRequests,
    intrinsic: Option<Vec2>,
    exited: bool,
    draft: Vec<u8>,
    requests: Vec<(Option<OpId>, Call)>,
    signatures: crate::signatures::Signatures,
    fetches: crate::fetches::Fetches,
    wakes: Arc<Wakes>,
    clock: std::time::Duration,
    desk: Option<desk::Desk>,
    desk_batches: Vec<InputBatch>,
    drawn_screens: Vec<(ScreenId, beui::WindowId)>,
    root: PhantomData<A>,
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
    Picker,
    Rename(Uuid),
}

impl<A: BeuiRoot> BeuiTest<A> {
    pub fn new(editor: OwnedEditor) -> Self {
        let block = editor.block_id();
        Self::open(
            Kind::Frame(block),
            Adopted::Editor(editor, None),
            Vec::new(),
        )
    }

    /// A test of the editor on `block`, with a default `EditorHost` that may edit.
    pub fn editing(block: Uuid) -> Self {
        let host = EditorHost::default();
        host.set_editable(true);
        Self::new(OwnedEditor::new(host, block))
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

    pub fn picker(host: EditorHost, request: be_sdk_beui::PickerRequest) -> Self {
        PICKER.with(|picker| *picker.borrow_mut() = Some(request));
        let editor = OwnedEditor::new(host, Uuid::nil());
        Self::open(Kind::Picker, Adopted::Editor(editor, None), Vec::new())
    }

    pub fn picker_beside<B: BeuiRoot>(
        beside: &BeuiTest<B>,
        host: EditorHost,
        request: be_sdk_beui::PickerRequest,
    ) -> Self {
        beside.store.forget_answers();
        SHARED_STORE.with(|shared| *shared.borrow_mut() = Some(beside.store.clone()));
        let mut test = Self::picker(host, request);
        if let Some(catalog) = beside.shown_catalog.clone() {
            test.catalog(catalog);
        }
        test
    }

    /// Another editor run beside `beside`, on the blocks it holds: the same block reopened, or
    /// one it made.
    pub fn new_beside<B: BeuiRoot>(beside: &BeuiTest<B>, editor: OwnedEditor) -> Self {
        beside.store.forget_answers();
        SHARED_STORE.with(|shared| *shared.borrow_mut() = Some(beside.store.clone()));
        Self::new(editor)
    }

    pub fn rename_beside<B: BeuiRoot>(beside: &BeuiTest<B>, host: EditorHost, block: Uuid) -> Self {
        beside.store.forget_answers();
        SHARED_STORE.with(|shared| *shared.borrow_mut() = Some(beside.store.clone()));
        let editor = OwnedEditor::new(host, Uuid::nil());
        let mut test = Self::open(
            Kind::Rename(block),
            Adopted::Editor(editor, None),
            Vec::new(),
        );
        if let Some(catalog) = beside.shown_catalog.clone() {
            test.catalog(catalog);
        }
        test
    }

    pub fn settings(artifacts: Artifacts, data: Vec<u8>) -> Self {
        Self::open(Kind::Settings, Adopted::Artifacts(artifacts), data)
    }

    /// A test of the editor shown for a revision of `source`: the editor's own block is the
    /// preview's, while `signing-block` names `source`, as the host does.
    pub fn in_revision_of(editor: OwnedEditor, source: Uuid) -> Self {
        let block = editor.block_id();
        Self::open_signing(
            Kind::Frame(block),
            Adopted::Editor(editor, None),
            Vec::new(),
            Some(source),
        )
    }

    fn open(kind: Kind, adopted: Adopted, data: Vec<u8>) -> Self {
        Self::open_signing(kind, adopted, data, None)
    }

    fn open_signing(kind: Kind, adopted: Adopted, data: Vec<u8>, signing: Option<Uuid>) -> Self {
        beui::native::verify_paint(true);
        beui::native::fail_unsettled_layout(true);
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
        let mut signatures = crate::signatures::Signatures::new(host.account_id());
        if let Some(source) = signing {
            signatures.set_signing_block(source);
        }
        let wakes = Arc::new(Wakes::default());
        let woken = Arc::clone(&wakes);
        host.waker().install(move || woken.wake());
        let mut app = HeadlessApp::new("block-ui-test", "block-ui-test", "0");
        app.adopt::<A>(INSTANCE, adopted);
        let store = SHARED_STORE
            .with(|shared| shared.borrow_mut().take())
            .unwrap_or_else(|| ContentStore::new(host.organization_id()));
        let blocks = host.graph();
        store.know_elsewhere(move |block| blocks.info(block).is_some());
        let block_type = host.block_type().unwrap_or_default();
        let (account_id, organization_id, client_id) = (
            host.account_id().into_bytes(),
            host.organization_id().into_bytes(),
            host.client_id().into_bytes(),
        );
        let opening_kind = match kind {
            Kind::Frame(block) | Kind::Preview(block) => {
                store.own(block, block_type);
                OpeningKind::Block(OpenBlock {
                    block_id: block.into_bytes(),
                    block_type: block_type.into_bytes(),
                    view_block: host.view_block().map(Uuid::into_bytes),
                    editable: host.editable(),
                    page_origin: host.page_origin(),
                    host_palette: host.host_palette(),
                })
            }
            Kind::Creation => OpeningKind::Creation(OpenCreation {
                template: template.clone(),
            }),
            Kind::Picker => {
                let request = PICKER
                    .with(|picker| picker.borrow_mut().take())
                    .expect("a picker test names what it picks");
                OpeningKind::Picker(be_sdk_api::OpenPicker {
                    filter: request.filter,
                    parent: request.parent.encode(),
                    sheet: request.sheet,
                })
            }
            Kind::Rename(block) => OpeningKind::Rename(be_sdk_api::OpenRename {
                block_id: block.into_bytes(),
                sheet: false,
            }),
            Kind::Settings => OpeningKind::Artifact(OpenArtifact {
                block_id: artifact.unwrap_or_default().0.into_bytes(),
                block_type: block_type.into_bytes(),
                source: artifact.unwrap_or_default().1.into_bytes(),
                data: data.clone(),
            }),
        };
        let open = Opening {
            instance: INSTANCE,
            editor: ADOPTED_EDITOR,
            account_id,
            organization_id,
            client_id,
            kind: opening_kind,
        };
        let frame = match &kind {
            Kind::Frame(_) => Some(be_sdk_api::FrameSpec {
                chrome: FrameChrome::Drawn,
                content: None,
                top_bar: TopBar::Hidden,
                dialog: None,
                anchor: None,
            }),
            Kind::Picker | Kind::Rename(_) => Some(be_sdk_api::FrameSpec {
                chrome: FrameChrome::None,
                content: None,
                top_bar: TopBar::Hidden,
                dialog: None,
                anchor: None,
            }),
            Kind::Creation => Some(be_sdk_api::FrameSpec {
                chrome: FrameChrome::None,
                content: None,
                top_bar: TopBar::Hidden,
                dialog: Some(be_sdk_api::DialogFrame {
                    modal: true,
                    ..be_sdk_api::DialogFrame::default()
                }),
                anchor: None,
            }),
            _ => None,
        };
        let mut test = Self {
            app,
            kind,
            host,
            store,
            size: SIZE,
            scale_factor: 1.0,
            raster: None,
            frame,
            steps: Vec::new(),
            gestures: Gestures::default(),
            frames: Vec::new(),
            inbox: Vec::new(),
            answered: Vec::new(),
            acknowledged: std::collections::HashSet::new(),
            connection: Connection::new(Side::Host),
            link: crate::link::Link::new(INSTANCE),
            editor: None,
            calls: Vec::new(),
            extension_calls: Vec::new(),
            web_views: std::collections::HashMap::new(),
            web_view_commands: Vec::new(),
            published: std::collections::HashMap::new(),
            published_watches: std::collections::HashMap::new(),
            child_statuses: be_sdk_api::Watch::default(),
            block_types: be_sdk_api::Watch::default(),
            shown_catalog: None,
            debug_templates: be_sdk_api::Watch::default(),
            completed: std::collections::HashMap::new(),
            menu: Vec::new(),
            docked: false,
            docks: Vec::new(),
            output: None,
            shown_window: None,
            drew: false,
            recording: None,
            overflows: Vec::new(),
            viewport: None,
            children: Vec::new(),
            views_children: Vec::new(),
            occluders: Vec::new(),
            report: None,
            actions: Vec::new(),
            window: be_sdk_api::WindowRequests::default(),
            intrinsic: None,
            exited: false,
            draft: data,
            requests: Vec::new(),
            signatures,
            fetches: crate::fetches::Fetches::default(),
            wakes,
            clock: std::time::Duration::ZERO,
            desk: None,
            desk_batches: Vec::new(),
            drawn_screens: Vec::new(),
            root: PhantomData,
        };
        roundtrip(test.app.hello(), "the app's hello");
        let accepted = roundtrip(
            HelloAccepted {
                host_name: "block-ui-test".to_owned(),
                surface: Some(SurfaceSpec {
                    format: SurfaceFormat::Rgba8UnormSrgb,
                    render: true,
                }),
                preferences: be_sdk_api::Preferences::default(),
            },
            "the host's acceptance",
        );
        test.deliver(Inbound::pushed(move |app| app.accept(accepted)));
        test.editor = Some(Editor::new(&mut test.connection, open));
        let frames = test.connection.take_outbound();
        test.deliver(Inbound::Frames(frames));
        test.place();
        test.run();
        // The editor's view has the keyboard, as one a person just opened would: an app holds
        // none until its host gives a view the keyboard (`blur` takes it away).
        test.focus();
        test.run();
        test
    }

    fn deliver(&mut self, inbound: Inbound) {
        let delivered = match inbound {
            Inbound::Pushed(deliver) => deliver(&mut self.app),
            Inbound::Frames(frames) => self.app.deliver(frames),
        };
        if let Err(error) = delivered {
            panic!("the app refused what the host sent: {error}");
        }
    }

    fn place(&mut self) {
        let shown = self.view();
        let raster = self.raster.unwrap_or(self.scale_factor);
        let metrics = ViewportMetrics {
            logical: be_sdk_api::Size {
                width: self.size.x,
                height: self.size.y,
            },
            visible: be_sdk_api::Pos2::ZERO,
            pixel_width: (self.size.x * raster).round() as u32,
            pixel_height: (self.size.y * raster).round() as u32,
            scale_factor: self.scale_factor,
            raster,
        };
        let screens = self.desk_requests().unwrap_or_else(|| {
            vec![ScreenRequest {
                screen: SCREEN,
                instance: INSTANCE,
                view: shown.id,
                placement: shown.placement,
                metrics,
                frame: self.frame,
                windows: false,
                host_place: None,
                origin: None,
                docked: self.docked,
                anchor_space: None,
                fit: false,
            }]
        });
        be_sdk_api::embed::exports::app::screens(&mut self.connection, screens);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
    }

    pub fn block_id(&self) -> Option<Uuid> {
        match self.kind {
            Kind::Frame(block) | Kind::Preview(block) => Some(block),
            Kind::Creation | Kind::Settings | Kind::Picker | Kind::Rename(_) => None,
        }
    }

    pub fn host(&self) -> &EditorHost {
        &self.host
    }

    pub fn store(&self) -> ContentStore {
        self.store.clone()
    }

    /// Gives the host a block's content: `None` for the editor's own, `Some(id)` for a block
    /// it reads with `content_of`, handed over only once the editor asks for it.
    pub fn hold<C: LiveEdit + Merge + Clone + Default>(&mut self, block: Option<Uuid>, content: C) {
        self.store.hold(block, content);
    }

    /// Like `hold`, with the content's parts after its first kept, for a type that loads only
    /// its first part and reads the rest by range.
    pub fn hold_parts<C: LiveEdit + Merge + Clone + Default>(
        &mut self,
        block: Option<Uuid>,
        content: C,
    ) {
        self.store.hold_parts(block, content);
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

    /// An edit arriving from someone else.
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
        self.call(|editor, connection| {
            editor.hear(connection, vec![EditorEvent::ArtifactSettings(data)]);
        });
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

    pub fn in_tab(mut self, tab: Option<Rect>) -> Self {
        let frame = self.frame.get_or_insert_with(Default::default);
        frame.anchor = tab;
        self.place();
        self.run();
        self
    }

    fn with_bar(mut self, closable: bool, top_bar: TopBar) -> Self {
        self.frame = Some(be_sdk_api::FrameSpec {
            chrome: FrameChrome::Drawn,
            content: closable.then_some(Rect::from_min_size(Pos2::ZERO, self.size)),
            top_bar,
            dialog: None,
            anchor: None,
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
        self.call(|editor, connection| {
            editor.hear(
                connection,
                vec![EditorEvent::MenuPick(MenuChoice {
                    view: ViewId::MAIN,
                    id,
                    anchor: None,
                })],
            );
        });
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
        self.call(|editor, connection| {
            editor.hear(connection, vec![EditorEvent::Dismiss]);
        });
        self.run();
    }

    pub fn set_dialog(&mut self, dialog: be_sdk_api::DialogFrame) {
        if let Some(frame) = &mut self.frame {
            frame.dialog = Some(dialog);
        }
        self.place();
        self.run();
    }

    pub fn handles_dismiss(&self) -> Option<bool> {
        self.calls.iter().rev().find_map(|call| match call {
            ShellMessage::HandlesDismiss(handles) => Some(*handles),
            _ => None,
        })
    }

    pub fn take_dialog_closes(&mut self) -> Vec<be_sdk_api::DialogAnswer> {
        self.take_shell_messages(|call| match call {
            ShellMessage::CloseDialog(answer) => Some(*answer),
            _ => None,
        })
    }

    pub fn take_child_focus(&mut self) -> Vec<ChildId> {
        self.take_shell_messages(|call| match call {
            ShellMessage::FocusChild(child) => Some(*child),
            _ => None,
        })
    }

    pub fn take_block_child_focus(&mut self) -> Vec<Uuid> {
        self.take_shell_messages(|call| match call {
            ShellMessage::FocusBlockChild(block) => Some(Uuid::from_bytes(*block)),
            _ => None,
        })
    }

    pub fn take_child_closes(&mut self) -> Vec<ChildId> {
        self.take_shell_messages(|call| match call {
            ShellMessage::CloseChild(child) => Some(*child),
            _ => None,
        })
    }

    pub fn take_child_dismissals(&mut self) -> Vec<ChildId> {
        self.take_shell_messages(|call| match call {
            ShellMessage::DismissChild(child) => Some(*child),
            _ => None,
        })
    }

    pub fn child_bar(&mut self, child: ChildId, action: BarAction) {
        self.call(|editor, connection| {
            editor.hear(
                connection,
                vec![EditorEvent::ChildBar(ChildBarPress { child, action })],
            );
        });
    }

    pub fn set_view(&mut self, view: Rect, scale: f32) {
        let rect = self.rect();
        let band = self.band(rect);
        if let Some(viewport) = &mut self.viewport {
            viewport.show(view, scale, band, rect.min);
        }
        self.call(|editor, connection| {
            editor.hear(
                connection,
                vec![EditorEvent::ViewChanged(ViewFrame { view, scale })],
            );
        });
    }

    pub fn move_view(&mut self, change: ViewChange) {
        let band = self.band(self.rect());
        self.viewport
            .as_mut()
            .expect("only an editor in a viewport has a view to move")
            .apply(change, band);
    }

    pub fn drag_block(&mut self, position: Pos2, block_id: Uuid, block_type: Uuid, dropped: bool) {
        self.drag_block_as(
            position,
            block_id,
            block_type,
            dropped,
            be_sdk_api::DropAction::Move,
        );
    }

    /// Drags a block over the editor as the host does, with `action` the
    /// modifiers held picked.
    pub fn drag_block_as(
        &mut self,
        position: Pos2,
        block_id: Uuid,
        block_type: Uuid,
        dropped: bool,
        action: be_sdk_api::DropAction,
    ) {
        let blocks = be_sdk_api::clipboard::Blocks {
            blocks: vec![be_sdk_api::clipboard::ClipBlock {
                id: block_id,
                organization: None,
                block_type,
                title: String::new(),
            }],
        };
        let drag = be_sdk_api::shell_types::DragOver {
            view: self.view().id,
            pos: position,
            items: vec![be_sdk_api::clipboard::ClipFormat::encode(&blocks)],
            action,
            dropped,
        };
        self.call(|editor, connection| {
            editor.hear(connection, vec![EditorEvent::Drag(Some(drag))]);
        });
    }

    pub fn docked(mut self) -> Self {
        self.docked = true;
        self.place();
        self.run();
        self
    }

    pub fn take_dock_messages(&mut self) -> Vec<be_sdk_api::DockMessage> {
        std::mem::take(&mut self.docks)
    }

    pub fn receive_dock_updates(&mut self, updates: Vec<be_sdk_api::DockUpdate>) {
        self.host.receive_dock_updates(updates);
        self.run();
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

    pub fn set_raster(&mut self, raster: f32) {
        self.raster = Some(raster);
        self.place();
        self.run();
    }

    pub fn block_types(&mut self, descriptors: Vec<BlockTypeDescriptor>) {
        self.catalog(Catalog {
            plain_files: Vec::new(),
            types: descriptors,
            templates: Vec::new(),
            editors: Vec::new(),
        });
    }

    pub fn catalog(&mut self, catalog: Catalog) {
        self.shown_catalog = Some(catalog.clone());
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

    pub fn drawn(&self) -> Option<&beui::native::WindowOutput> {
        self.shown_output().filter(|_| self.drew)
    }

    pub fn test_ids(&self) -> std::rc::Rc<beui::native::accessibility_dump::TestIds> {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_ids()
    }

    fn shown_output(&self) -> Option<&beui::native::WindowOutput> {
        self.output.as_ref()?.window(self.shown_window?)
    }

    fn shown_changed(&self) -> bool {
        self.shown_output().is_some_and(|shown| shown.changed)
    }

    pub fn document(&self) -> std::cell::Ref<'_, Document> {
        self.app
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
        Some(report.content?.inside(self.rect()))
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
            .expect("the app never answered the replacement");
        ticket
            .decode(outcome)
            .expect("the app answered the replacement")
    }

    pub fn presence_visible(&mut self, visible: bool) {
        self.call(|editor, connection| {
            editor.hear(connection, vec![EditorEvent::Presence(visible)]);
        });
        self.run();
    }

    pub fn resize(&mut self, size: Vec2) {
        let size = be_sdk_api::Size {
            width: size.x,
            height: size.y,
        };
        self.call(|editor, connection| {
            editor.hear(connection, vec![EditorEvent::Resized(size)]);
        });
        self.run();
    }

    pub fn set_histories(
        &mut self,
        states: impl IntoIterator<Item = (Uuid, be_sdk_beui::BlockHistory)>,
    ) {
        for (block, history) in states {
            self.link
                .offer_history(block, &crate::link::history(history));
        }
    }

    pub fn set_version_status(&mut self, status: be_sdk_beui::VersionStatus) {
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

    pub fn receive_from_peer<M: be_sdk_beui::be_block::presence::ChannelKind>(
        &mut self,
        from: u64,
        message: &M,
    ) {
        self.block_id()
            .expect("this editor has no block of its own to hear peers on");
        self.store
            .receive_from_peer(None, from, M::ID, message.encode());
    }

    pub fn web_view_event(&mut self, web_view: WebViewId, event: be_sdk_beui::WebViewEvent) {
        if let Some((_, watch)) = self.web_views.values_mut().find(|(id, _)| *id == web_view) {
            watch.change(&mut self.connection, |held| {
                let mut events = held.unwrap_or_default();
                be_sdk_beui::WebViewEvent::queue(&mut events, event);
                events
            });
            let frames = self.connection.take_outbound();
            self.inbox.push(Inbound::Frames(frames));
        }
    }

    pub fn set_preferences(&mut self, preferences: be_sdk_api::Preferences) {
        let preferences = roundtrip(preferences, "the host's preferences");
        self.inbox
            .push(Inbound::pushed(move |app| app.set_preferences(preferences)));
    }

    pub fn set_watched<W: be_sdk_api::Watched>(&mut self, value: &W::Value) {
        let value = be_sdk_api::ops::encode(value);
        for (open, watch) in self.published_watches.values_mut() {
            if *open == W::OPEN {
                watch.set(&mut self.connection, value.clone());
            }
        }
        self.published.insert(W::OPEN, value);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
    }

    pub fn watches<W: be_sdk_api::Watched>(&self) -> bool {
        self.published_watches
            .values()
            .any(|(open, _)| *open == W::OPEN)
    }

    /// How `verify` answers a signature `account` made (`be_sdk_test::signature`); a contact's,
    /// `Signer::Verified`, unless set. A signature it did not make is always `Signer::Bad`.
    pub fn set_signer(&mut self, account: Uuid, signer: be_sdk_api::Signer) {
        self.signatures.set(account, signer);
    }

    pub fn answer<T, E: crate::wire::Encode<T>>(&mut self, reply: Reply<T, E>, answer: T) {
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
        let view = self.view().id;
        self.report_children(|placement| available_status(view, placement));
    }

    pub fn every_child(&self) -> Vec<ChildPlacement> {
        self.views_children
            .iter()
            .flat_map(|(_, children)| children.iter().cloned())
            .collect()
    }

    pub fn report_child(&mut self, child: ChildId, change: impl FnOnce(&mut ChildStatus)) {
        let (view, placement) = self
            .views_children
            .iter()
            .find_map(|(view, children)| {
                children
                    .iter()
                    .find(|placement| placement.child == child)
                    .map(|placement| (*view, placement))
            })
            .expect("the editor placed the child to report on");
        let mut status = available_status(view, placement);
        change(&mut status);
        self.child_statuses.change(&mut self.connection, |held| {
            let mut held = held.unwrap_or_default();
            held.retain(|held| held.child != child);
            held.push(status);
            held
        });
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size)
    }

    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Paints the queued frames, then applies the editor's operations to what the host holds
    /// and sends them back marked as its own, painting again until nothing changes.
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

    /// Moves the test's clock on and paints. The editor's frame clock is this clock, one frame
    /// interval per painted frame, so timers cost no real time and land on the same frame on
    /// every machine.
    pub fn advance(&mut self, by: std::time::Duration) {
        self.clock += by;
        self.run();
    }

    /// Moves the clock on without painting and hands the app what the host queued, as a call
    /// reaches an idle editor between frames; the next `run()` paints the frame that takes it.
    pub fn wait(&mut self, by: std::time::Duration) {
        self.clock += by;
        for message in std::mem::take(&mut self.inbox) {
            self.deliver(message);
        }
        self.hear();
    }

    /// Paints until a frame stops asking to be painted again at once. Call it before a
    /// snapshot of something that animates in.
    pub fn settle(&mut self) {
        self.settle_until("its animations to finish", |test| {
            test.output.as_ref().is_none_or(|output| {
                !test.shown_changed() && output.repaint_after > std::time::Duration::ZERO
            })
        });
    }

    /// Paints until `ready` holds, failing with `what` at a deadline. Between frames it waits
    /// as be-host does, for the editor's `Waker` or the delay it asked for, never a fixed
    /// time; use it for work on another thread, where a few more `run()`s only widen a race.
    /// Work that lands without waking the editor stalls it until the deadline, as it would
    /// in be-host.
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

    /// Paints one frame with exactly these events, ignoring what gestures queued.
    pub fn step(&mut self, events: Vec<Input>) {
        self.store.tick(self.clock.as_millis() as u64);
        let mut inbox = std::mem::take(&mut self.inbox);
        self.link.answer(&self.store, &mut self.connection);
        self.fetches.answer(&mut self.connection);
        let mut frames = std::mem::take(&mut self.answered);
        frames.extend(self.connection.take_outbound());
        if !frames.is_empty() {
            inbox.push(Inbound::Frames(frames));
        }
        let rect = self.rect();
        let band = self.band(rect);
        let intrinsic = self.intrinsic;
        if let Some((view, scale)) = self
            .viewport
            .as_mut()
            .and_then(|viewport| viewport.place(band, rect.min, intrinsic))
        {
            inbox.push(self.framed(|editor, connection| {
                editor.hear(
                    connection,
                    vec![EditorEvent::ViewChanged(ViewFrame { view, scale })],
                );
            }));
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
                        over: None,
                    }
                })
                .collect();
            let batch = roundtrip(
                InputBatch {
                    screen: SCREEN,
                    every_sample: false,
                    events,
                    dismissed: Vec::new(),
                },
                "the host's input",
            );
            inbox.push(Inbound::pushed(move |app| app.input(batch)));
        }
        for batch in std::mem::take(&mut self.desk_batches) {
            let batch = roundtrip(batch, "the host's input");
            inbox.push(Inbound::pushed(move |app| app.input(batch)));
        }
        let now_micros = self.clock.as_micros() as u64;
        let wall_micros = (beui_core::timer::TEST_WALL_CLOCK + self.clock).as_micros() as u64;
        inbox.push(Inbound::pushed(move |app| {
            app.draw_frame(now_micros, wall_micros, true)
        }));
        for message in inbox {
            self.deliver(message);
        }
        self.hear();
        let drawn = self.app.draw().and_then(|drawn| {
            self.drawn_screens = drawn
                .windows
                .iter()
                .map(|(placement, window)| (placement.screen, *window))
                .collect();
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
        self.desk_escaped();
        if let Some(commit) = self.app.outbound() {
            self.committed(roundtrip(commit, "the app's frame"));
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
            let framed = self.framed(|editor, connection| {
                editor.hear(
                    connection,
                    vec![EditorEvent::ViewChanged(ViewFrame { view, scale })],
                );
            });
            self.inbox.push(framed);
        }
    }

    fn committed(&mut self, commit: LayoutCommit) {
        self.docks.extend(commit.docks);
        let view = self.view().id;
        let asked = commit.window_requests;
        let reports = commit.reports.clone();
        for placements in &commit.children {
            self.views_children
                .retain(|(held, _)| *held != placements.view);
            self.views_children
                .push((placements.view, placements.children.clone()));
        }
        if let Some(placements) = commit
            .children
            .into_iter()
            .filter(|placements| placements.view == view)
            .last()
        {
            let hidden = |placement: &ChildPlacement| placement.mode == ChildMode::Hidden;
            self.occluders = placements
                .occluders
                .into_iter()
                .map(|occluder| Occluder {
                    after: placements.children
                        [..(occluder.after as usize).min(placements.children.len())]
                        .iter()
                        .filter(|placement| !hidden(placement))
                        .count() as u32,
                    ..occluder
                })
                .collect();
            self.children = placements
                .children
                .into_iter()
                .filter(|placement| !hidden(placement))
                .collect();
        }
        if let Some(mut report) = commit
            .reports
            .into_iter()
            .find(|report| report.screen == SCREEN)
        {
            if let Some(actions) = report.actions.take() {
                self.actions = actions;
            }
            self.report = Some(report);
        }
        self.desk_intercepts(&commit.windows);
        if let Some(window) = commit
            .windows
            .into_iter()
            .find(|window| window.screen == SCREEN)
            .filter(|_| self.view().is_main())
        {
            self.window = window.window;
        }
        self.desk_asked(asked, &reports);
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
        for frame in self.app.take_frames() {
            self.connection.receive(&frame).unwrap_or_else(|error| {
                panic!("the app sent a frame that did not decode: {error}")
            });
        }
        while let Some(event) = self.connection.next_event() {
            let call = match event {
                Incoming::Started(started) if published(started.method).is_some() => {
                    let connection = &mut self.connection;
                    let watching = published(started.method).unwrap_or_else(|| unreachable!());
                    if watching.open == started.method {
                        let mut watch = be_sdk_api::Watch::default();
                        watch.opened(started.op);
                        if let Some(value) = self.published.get(&watching.open)
                            && !be_sdk_api::starts_anew(watching.open)
                        {
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
                Incoming::Canceled { op } if self.link.holds(op) => {
                    self.link.ended(op, &self.store, &mut self.connection);
                    continue;
                }
                Incoming::Canceled { op } if self.fetches.holds(op) => {
                    self.fetches.ended(op, &mut self.connection);
                    continue;
                }
                Incoming::Canceled { op } if self.fetches.withdrawn(op, &mut self.connection) => {
                    continue;
                }
                Incoming::Started(started) if extension_call(started.method) => {
                    self.connection.complete(started.op, Outcome::returned(&()));
                    self.acknowledged.insert(started.op);
                    self.extension_calls.push(started);
                    continue;
                }
                Incoming::Completed { op, outcome } => {
                    self.completed.insert(op, outcome);
                    continue;
                }
                Incoming::Started(started) => {
                    let op = started.op;
                    let shown = self.block_id();
                    let connection = &mut self.connection;
                    match embed::Call::accept(connection, started) {
                        Some(embed::Call::Shell(call)) => match call {
                            ShellCall::ChildStatusesWatchNew { resource, .. } => {
                                self.child_statuses.opened(resource.op());
                                continue;
                            }
                            ShellCall::ChildStatusesWatchNext { reply, .. } => {
                                self.child_statuses.next(connection, reply);
                                continue;
                            }
                            ShellCall::BlockTypesWatchNew { resource } => {
                                self.block_types.opened(resource.op());
                                continue;
                            }
                            ShellCall::BlockTypesWatchNext { reply, .. } => {
                                self.block_types.next(connection, reply);
                                continue;
                            }
                            ShellCall::WebViewNew {
                                resource, id, url, ..
                            } => {
                                let mut watch = be_sdk_api::Watch::default();
                                watch.opened(resource.op());
                                self.web_views.insert(resource.op(), (id, watch));
                                self.web_view_commands.push((id, WebViewCommand::Open(url)));
                                continue;
                            }
                            ShellCall::WebViewNextEvents { this, reply } => {
                                match self.web_views.get_mut(&this.op()) {
                                    Some((_, watch)) => watch.next(connection, reply),
                                    None => reply.fail(connection, OpError::Canceled),
                                }
                                continue;
                            }
                            call => {
                                connection.complete(op, Outcome::returned(&()));
                                self.acknowledged.insert(op);
                                call
                            }
                        },
                        Some(embed::Call::Debug(call)) => {
                            match call {
                                DebugCall::DebugTemplatesWatchNew { resource } => {
                                    self.debug_templates.opened(resource.op())
                                }
                                DebugCall::DebugTemplatesWatchNext { reply, .. } => {
                                    self.debug_templates.next(connection, reply)
                                }
                                DebugCall::Performance { reply, .. }
                                | DebugCall::SimulateAccess { reply, .. } => {
                                    reply.send(connection, &());
                                    self.acknowledged.insert(op);
                                }
                            }
                            continue;
                        }
                        Some(embed::Call::Blocks(call)) => {
                            if call.changes() {
                                self.acknowledged.insert(op);
                            }
                            let author = self.host.account_id();
                            self.link.call(call, &self.store, connection, author);
                            continue;
                        }
                        Some(embed::Call::HostServices(call)) => {
                            if let Some(call) = self.signatures.answer(call, connection, shown) {
                                self.requests.push((Some(op), call));
                            }
                            continue;
                        }
                        Some(embed::Call::Network(call)) => {
                            self.fetches.call(call, connection);
                            continue;
                        }
                        Some(call) => {
                            let (op, outcome) = call.refuse(OpError::Unsupported);
                            connection.complete(op, outcome);
                            continue;
                        }
                        None => continue,
                    }
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
            let shown = |this: &be_sdk_api::embed::shell::WebView| {
                self.web_views.get(&this.op()).map(|(id, _)| *id)
            };
            let commanded = match &call {
                ShellCall::WebViewLoad { this, url, .. } => {
                    shown(this).map(|id| (id, WebViewCommand::Load(url.clone())))
                }
                ShellCall::WebViewReload { this, .. } => {
                    shown(this).map(|id| (id, WebViewCommand::Reload))
                }
                ShellCall::WebViewFocusHost { this, .. } => {
                    shown(this).map(|id| (id, WebViewCommand::FocusHost))
                }
                _ => None,
            };
            if let Some(commanded) = commanded {
                self.web_view_commands.push(commanded);
                continue;
            }
            let ShellCall::Tell { messages, .. } = call else {
                continue;
            };
            for message in messages {
                match &message {
                    ShellMessage::Menu(MenuSet { view, items }) if view.is_main() => {
                        self.menu = MenuEntry::nest(items.clone()).unwrap_or_else(|error| {
                            panic!("the app sent a menu it may not: {error}")
                        });
                    }
                    ShellMessage::LeaveFrame => self.exited = true,
                    ShellMessage::ArtifactEdited(data) => self.draft.clone_from(data),
                    ShellMessage::IntrinsicSize(size) => {
                        self.intrinsic = size.map(|size| Vec2::new(size.width, size.height));
                    }
                    _ => {}
                }
                self.calls.push(message);
            }
        }
        self.link.answer(&self.store, &mut self.connection);
        self.fetches.answer(&mut self.connection);
        let frames = self.connection.take_outbound();
        let acknowledged = &mut self.acknowledged;
        match frames.iter().all(|frame| acknowledges(frame, acknowledged)) {
            true => self.answered.extend(frames),
            false => {
                let mut answered = std::mem::take(&mut self.answered);
                answered.extend(frames);
                self.inbox.push(Inbound::Frames(answered));
            }
        }
    }

    /// Takes the shell messages `pick` accepts, in the order the editor told them; the rest wait.
    pub fn take_shell_messages<T>(&mut self, pick: impl Fn(&ShellMessage) -> Option<T>) -> Vec<T> {
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

    pub fn take_shell_commands(&mut self) -> Vec<(Uuid, be_sdk_api::ShellCommand)> {
        self.take_shell_messages(|call| match call {
            ShellMessage::Act(BlockAct { block, command }) => {
                Some((Uuid::from_bytes(*block), *command))
            }
            _ => None,
        })
    }

    /// Every copy the editor made: its serial and the items it offered.
    pub fn take_copies(&mut self) -> Vec<(u64, Vec<be_sdk_api::OfferedClipItem>)> {
        self.take_shell_messages(|call| match call {
            ShellMessage::Copy(ClipOffer { serial, items }) => Some((*serial, items.clone())),
            _ => None,
        })
    }

    /// The `format` data of each copy that offered it ready.
    pub fn take_copied(&mut self, format: &str) -> Vec<Vec<u8>> {
        self.take_shell_messages(|call| match call {
            ShellMessage::Copy(ClipOffer { items, .. }) => items
                .iter()
                .find(|item| item.format == format)
                .and_then(|item| item.data.clone()),
            _ => None,
        })
    }

    /// The plain text of each copy that offered it.
    pub fn take_copied_text(&mut self) -> Vec<String> {
        self.take_copied(be_sdk_api::clipboard::formats::TEXT)
            .into_iter()
            .map(|text| String::from_utf8_lossy(&text).into_owned())
            .collect()
    }

    pub fn take_copied_images(&mut self) -> Vec<Vec<u8>> {
        self.take_copied(be_sdk_api::clipboard::formats::PNG)
    }

    pub fn placed_picker(&self) -> Option<ChildPlacement> {
        self.placed(|content| matches!(content, be_sdk_api::ChildContent::Picker { .. }))
    }

    pub fn placed(
        &self,
        which: impl Fn(&be_sdk_api::ChildContent) -> bool,
    ) -> Option<ChildPlacement> {
        self.children
            .iter()
            .find(|placement| which(&placement.content))
            .cloned()
    }

    pub fn answer_picker(&mut self, pick: be_sdk_beui::BlockPick) {
        let view = self.view().id;
        let picker = self
            .placed_picker()
            .expect("the editor placed a picker to answer")
            .child;
        self.report_children(|placement| ChildStatus {
            instance: INSTANCE,
            view,
            child: placement.child,
            available: true,
            intrinsic: None,
            active: false,
            interaction: be_sdk_beui::InteractionMode::Live,
            capabilities: be_sdk_beui::EditorCapabilities::default(),
            resize: be_sdk_beui::ResizeMode::None,
            error: None,
            menu: Vec::new(),
            creation: None,
            preview: Default::default(),
            dialog: Default::default(),
            picked: (placement.child == picker).then(|| pick.clone()),
            card: false,
            title: None,
            icon: None,
            group: None,
            raised: 0,
            rung: 0,
            minimized: 0,
            fullscreen: false,
            notice: None,
            menu_asked: None,
        });
        self.run();
    }

    pub fn show_picker(&mut self, intrinsic: Vec2, card: bool) {
        let view = self.view().id;
        let picker = self
            .placed_picker()
            .expect("the editor placed a picker to show")
            .child;
        self.report_children(|placement| {
            let shown = placement.child == picker;
            ChildStatus {
                instance: INSTANCE,
                view,
                child: placement.child,
                available: true,
                intrinsic: shown.then_some(be_sdk_api::Size {
                    width: intrinsic.x,
                    height: intrinsic.y,
                }),
                active: false,
                interaction: be_sdk_beui::InteractionMode::Live,
                capabilities: be_sdk_beui::EditorCapabilities::default(),
                resize: be_sdk_beui::ResizeMode::None,
                error: None,
                menu: Vec::new(),
                creation: None,
                preview: Default::default(),
                dialog: Default::default(),
                picked: None,
                card: shown && card,
                title: None,
                icon: None,
                group: None,
                raised: 0,
                rung: 0,
                minimized: 0,
                fullscreen: false,
                notice: None,
                menu_asked: None,
            }
        });
        self.run();
    }

    /// The actions the editor's view last described to the host (`frame-report.actions`), sent
    /// whether or not the host asks it to describe, as be-host's palette lists them.
    pub fn described_actions(&self) -> &[be_sdk_api::DescribedAction] {
        &self.actions
    }

    pub fn described_action(&self, id: &str) -> be_sdk_api::DescribedAction {
        self.actions
            .iter()
            .find(|action| action.id == id)
            .cloned()
            .unwrap_or_else(|| panic!("the view describes no action {id:?}"))
    }

    /// Runs an action by id as the host's palette does (`app.run-action`).
    pub fn run_action(&mut self, id: &str) {
        be_sdk_api::embed::exports::app::run_action(&mut self.connection, SCREEN, id.to_owned());
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
        self.run();
    }

    /// The palettes the editor asked the host to open (`shell-message.open-palette`).
    pub fn take_palette_opens(&mut self) -> Vec<be_sdk_api::PaletteMode> {
        self.take_shell_messages(|call| match call {
            ShellMessage::OpenPalette(mode) => Some(*mode),
            _ => None,
        })
    }

    pub fn take_cards(&mut self) -> Vec<bool> {
        self.take_shell_messages(|call| match call {
            ShellMessage::ShowCard(card) => Some(*card),
            _ => None,
        })
    }

    pub fn take_picked(&mut self) -> Vec<be_sdk_beui::BlockPick> {
        self.take_shell_messages(|call| match call {
            ShellMessage::Picked(pick) => Some(pick.clone()),
            _ => None,
        })
    }

    pub fn show_dialog(&mut self, block: Uuid, dialog: be_sdk_api::ShellDialog) {
        self.call(|editor, connection| {
            editor.hear(
                connection,
                vec![EditorEvent::ShowDialog(DialogRequest {
                    block: block.into_bytes(),
                    dialog,
                })],
            );
        });
    }

    pub fn take_renames(&mut self) -> Vec<(Uuid, Option<String>)> {
        self.take_sent_calls(|sent| match sent {
            Sent::Define(block, key, value) if *key == be_sdk_beui::be_block::NAME.id => Some((
                *block,
                value
                    .as_ref()
                    .and_then(|value| be_sdk_beui::be_block::be_block_model::from_cbor(value)),
            )),
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

    pub fn take_access_changes(&mut self) -> Vec<(Uuid, Uuid, be_sdk_api::AccessLevel)> {
        self.take_requests_where(|call| match call {
            Call::SetAccess {
                block,
                account,
                access,
                ..
            } => Ok((Uuid::from_bytes(block), Uuid::from_bytes(account), access)),
            call => Err(call),
        })
    }

    /// What the editor's replica announced to the block's other members once the server held an
    /// edit (`LiveEdit::announcements`): the block, the text and the accounts it mentions.
    pub fn take_announced(&mut self) -> Vec<(Uuid, String, Vec<Uuid>)> {
        self.store.take_notified()
    }

    /// What the editor asked of pushes to this device (`host-services.set-push-device`): true
    /// for on.
    pub fn take_push_device_requests(&mut self) -> Vec<bool> {
        self.take_requests_where(|call| match call {
            Call::SetPushDevice { on, .. } => Ok(on),
            call => Err(call),
        })
    }

    pub fn take_child_commits(
        &mut self,
    ) -> Vec<(ChildId, be_sdk_api::BlockLocation, Option<String>)> {
        self.take_shell_messages(|call| match call {
            ShellMessage::CommitChild(ChildCreation {
                child,
                parent,
                name,
            }) => Some((*child, *parent, name.clone())),
            _ => None,
        })
    }

    pub fn take_audio_commands(&mut self) -> Vec<be_sdk_api::AudioCommand> {
        self.take_shell_messages(|call| match call {
            ShellMessage::PlayAudio(command) => Some(command.clone()),
            _ => None,
        })
    }

    pub fn take_view_changes(&mut self) -> Vec<ViewChange> {
        self.take_shell_messages(|call| match call {
            ShellMessage::ChangeView(change) => Some(*change),
            _ => None,
        })
    }

    pub fn take_child_menu_picks(&mut self) -> Vec<(ChildId, String)> {
        self.take_shell_messages(|call| match call {
            ShellMessage::ChildMenuPick(ChildMenuChoice { child, id, .. }) => {
                Some((*child, id.clone()))
            }
            _ => None,
        })
    }

    pub fn take_bar_actions(&mut self) -> Vec<BarAction> {
        self.take_shell_messages(|call| match call {
            ShellMessage::BarAction(action) => Some(*action),
            _ => None,
        })
    }

    pub fn has_requests(&self) -> bool {
        !self.requests.is_empty()
    }

    /// Whether the editor sent a fetch `take_fetches` has not taken yet.
    pub fn has_fetches(&self) -> bool {
        self.fetches.has_sent()
    }

    /// The requests the editor sent with `EditorHost::fetch` since the last call, each
    /// whole as it asked; answer each with `respond` or `fail_fetch`.
    pub fn take_fetches(&mut self) -> Vec<crate::Fetch> {
        self.fetches.take_sent()
    }

    /// Answers `fetch` with `head`, then its body in `chunks`, each sent once the editor has
    /// credit for it, as a host streams a body that arrives in pieces.
    pub fn respond(
        &mut self,
        fetch: &crate::Fetch,
        head: be_sdk_api::network::ResponseHead,
        chunks: impl IntoIterator<Item = Vec<u8>>,
    ) {
        self.fetches.respond(fetch, head, chunks);
        self.fetches.answer(&mut self.connection);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
    }

    /// Answers `fetch` with `failure`: no response came, or the body ends with it.
    pub fn fail_fetch(&mut self, fetch: &crate::Fetch, failure: be_sdk_beui::Failure) {
        self.fetches.fail(fetch, failure);
        self.fetches.answer(&mut self.connection);
        let frames = self.connection.take_outbound();
        self.inbox.push(Inbound::Frames(frames));
    }

    /// The links the editor opened with `EditorHost::open_url` since the last call.
    pub fn take_opened_urls(&mut self) -> Vec<String> {
        std::mem::take(&mut self.fetches.opened)
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

    pub fn take_opens(&mut self) -> Vec<be_sdk_beui::OpenRequest> {
        self.take_sent_calls(|sent| match sent {
            Sent::Open(open) => Some(be_sdk_beui::OpenRequest {
                block_id: Uuid::from_bytes(open.block_id),
                block_type: Uuid::from_bytes(open.block_type),
                via: open.via.map(Uuid::from_bytes),
                editor: open.editor.map(Uuid::from_bytes),
            }),
            _ => None,
        })
    }

    pub fn take_block_commands(&mut self) -> Vec<(Uuid, be_sdk_beui::BlockCommand)> {
        self.take_sent_calls(|sent| match sent {
            Sent::Command(block, command) => Some((*block, *command)),
            _ => None,
        })
    }

    pub fn take_host_calls<C>(
        &mut self,
        accept: impl Fn(&be_sdk_api::ops::Started) -> Option<C>,
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

    pub fn take_version_commands(&mut self) -> Vec<(Uuid, be_sdk_beui::VersionCommand)> {
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

    pub fn take_sent_to_peers<M: be_sdk_beui::be_block::presence::ChannelKind>(
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
        self.take_shell_messages(|call| match call {
            ShellMessage::DragAccepted(accepted) => Some(*accepted),
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

    fn view(&self) -> View {
        match self.kind {
            Kind::Preview(_) => View::PREVIEW,
            Kind::Settings => View::ARTIFACT_SETTINGS,
            Kind::Frame(_) | Kind::Creation | Kind::Picker | Kind::Rename(_) => View::MAIN,
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
        self.drive("back", &[], &[]);
    }

    pub fn handles_back(&self) -> bool {
        self.window.handles_back
    }

    pub fn wants_keyboard(&self) -> bool {
        self.window.wants_keyboard
    }

    pub fn window(&self) -> &be_sdk_api::WindowRequests {
        &self.window
    }

    pub fn intercepted_keys(&self) -> Vec<KeyChord> {
        self.window.intercepted_keys.clone()
    }

    pub fn host_key(&mut self, modifiers: Modifiers, key: Key) -> bool {
        self.hold_host_key(modifiers, key, 0)
    }

    pub fn hold_host_key(&mut self, modifiers: Modifiers, key: Key, repeats: usize) -> bool {
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

    pub fn host_tap(&mut self, key: Key) -> bool {
        if let Some(tapped) = self.desk_tap(key) {
            return tapped;
        }
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

    pub fn accessible_description(&self, test_id: &str) -> Option<String> {
        let document = self.document();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
        document
            .accessibility
            .get(&node)?
            .description()
            .map(str::to_owned)
    }

    pub fn rect_of(&self, test_id: &str) -> Rect {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_id_rect(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}\n{}", self.replay()))
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
        let cut = crate::document::scroll_cuts(&self.document(), test_id);
        let painted = self
            .output
            .as_ref()
            .and_then(|output| output.test_id_rect(test_id));
        if !cut
            && painted.is_some_and(|rect| self.desk.is_some() || self.rect().contains_rect(rect))
        {
            return;
        }
        {
            let mut document = self
                .app
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
        self.down(from, PointerButton::Secondary);
        self.next_frame();
        self.up(to, PointerButton::Secondary);
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
        self.steps.push(format!(
            "drive pinch {} {factor}",
            Target::point(pos).word(1.0)
        ));
        let anchor = Pos2::ZERO + (pos - self.rect().min);
        self.calls.push(ShellMessage::ChangeView(ViewChange::Zoom {
            factor,
            anchor: Some(anchor),
        }));
        self.run();
    }

    pub fn predict(&mut self, pos: Pos2) {
        self.push(Input::Predicted(pos));
    }

    pub fn hover(&mut self, target: impl Into<Target>) {
        self.drive_at("move", &[target.into()], &[]);
    }

    pub fn click(&mut self, target: impl Into<Target>) {
        self.drive_at("click", &[target.into()], &[]);
    }

    pub fn click_with(&mut self, target: impl Into<Target>, button: PointerButton) {
        self.drive_at("click", &[target.into()], &[button_name(button)]);
    }

    pub fn double_click(&mut self, target: impl Into<Target>) {
        self.drive_at("click", &[target.into()], &["double".to_owned()]);
    }

    pub fn tap(&mut self, target: impl Into<Target>) {
        self.drive_at("tap", &[target.into()], &[]);
    }

    pub fn down(&mut self, target: impl Into<Target>, button: PointerButton) {
        self.drive_at("down", &[target.into()], &[button_name(button)]);
    }

    pub fn up(&mut self, target: impl Into<Target>, button: PointerButton) {
        self.drive_at("up", &[target.into()], &[button_name(button)]);
    }

    pub fn drag(&mut self, from: impl Into<Target>, to: impl Into<Target>) {
        self.drive_at("drag", &[from.into(), to.into()], &[]);
    }

    /// Spends a frame on each point and releases in a frame of its own, for a gesture whose
    /// path matters (a lasso) or that should come to rest where it ends.
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

    /// Takes the keyboard from the editor's view, as when its window loses the focus (`drive
    /// blur`); `focus` gives it back.
    pub fn blur(&mut self) {
        self.drive("blur", &[], &[]);
    }

    pub fn focus(&mut self) {
        self.drive("focus", &[], &[]);
    }

    pub fn scroll(&mut self, target: impl Into<Target>, by: Vec2) {
        self.drive_at(
            "scroll",
            &[target.into()],
            &[by.y.to_string(), by.x.to_string()],
        );
    }

    pub fn fingers<T: Into<Target> + Clone>(&mut self, phase: TouchPhase, fingers: &[(u64, T)]) {
        let targets: Vec<Target> = fingers
            .iter()
            .map(|(_, target)| target.clone().into())
            .collect();
        let phase = match phase {
            TouchPhase::Start => "down",
            TouchPhase::Move => "move",
            TouchPhase::End => "up",
            TouchPhase::Cancel => "cancel",
        };
        let words: Vec<String> = std::iter::once(phase.to_owned())
            .chain(
                fingers
                    .iter()
                    .zip(&targets)
                    .map(|((finger, _), target)| format!("{finger}={}", target.word(1.0))),
            )
            .collect();
        self.drive("touch", &words, &targets);
    }

    pub fn type_text(&mut self, text: &str) {
        self.drive("type", &[text.to_owned()], &[]);
    }

    pub fn key(&mut self, key: Key, modifiers: Modifiers) {
        let chord = beui::input::typing::chord_name(key, modifiers)
            .unwrap_or_else(|| panic!("drive has no name for {modifiers:?}+{key:?}"));
        self.drive("key", &[chord], &[]);
    }

    pub fn key_down(&mut self, key: Key) {
        self.held_key(key, true);
    }

    pub fn key_up(&mut self, key: Key) {
        self.held_key(key, false);
    }

    fn held_key(&mut self, key: Key, pressed: bool) {
        let command = if pressed { "keydown" } else { "keyup" };
        let name = key
            .name()
            .unwrap_or_else(|| panic!("drive has no name for {key:?}"));
        self.drive(command, &[name], &[]);
    }

    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    fn drive_at(&mut self, command: &str, targets: &[Target], rest: &[String]) {
        let words: Vec<String> = targets
            .iter()
            .map(|target| target.word(1.0))
            .chain(rest.iter().cloned())
            .collect();
        self.drive(command, &words, targets);
    }

    fn drive(&mut self, command: &str, arguments: &[String], targets: &[Target]) {
        let mut step = format!("drive {command}");
        for word in arguments {
            let plain = !word.is_empty()
                && word
                    .chars()
                    .all(|character| character.is_ascii_graphic() && !"\"'\\".contains(character));
            match plain {
                true => step.push_str(&format!(" {word}")),
                false => step.push_str(&format!(" {word:?}")),
            }
        }
        self.steps.push(step);
        let points: Vec<(String, Pos2)> = targets
            .iter()
            .map(|target| (target.word(1.0), self.point(target.clone())))
            .collect();
        let at = |word: &str| {
            points
                .iter()
                .find(|(named, _)| named == word)
                .map(|(_, at)| *at)
                .ok_or_else(|| format!("{word} is not one of the targets"))
        };
        let frames = drive::plan(&mut self.gestures, command, arguments, 1.0, &at)
            .and_then(|frames| frames.ok_or_else(|| format!("{command} gives no input")))
            .unwrap_or_else(|error| panic!("{error}\n{}", self.replay()));
        match self.desk.is_some() && matches!(command, "click" | "move" | "down" | "up" | "drag") {
            true => self.desk_perform(frames),
            false => self.perform(frames),
        }
    }

    fn point(&mut self, target: Target) -> Pos2 {
        match target {
            Target::Id(id, offset) if self.desk.is_some() => {
                self.reveal(&id);
                self.desk_point(&id, offset).unwrap_or_else(|| {
                    panic!(
                        "no element with test id {id:?} on the desk\n{}",
                        self.replay()
                    )
                })
            }
            Target::Id(id, offset) => {
                self.reveal(&id);
                match offset {
                    Some(offset) => self.rect_of(&id).min + offset,
                    None => self.point_of(&id),
                }
            }
            Target::Point(at) => at,
            Target::Text(text) => panic!(
                "an app test finds a target by its test id or a point, not by text: {text:?}\n{}",
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
        self.note_overflows();
        match &mut self.recording {
            Some(recording) => recording.append(frame),
            None => self.recording = Some(frame),
        }
    }

    pub fn snapshot(&mut self, name: &str) {
        let painting = match self.recording.take() {
            Some(recording) => recording,
            None => {
                self.note_overflows();
                self.painted()
            }
        };
        beui_paint_snapshot::assert_accepted(name, &painting, beui_paint_snapshot::HOW_TO_ACCEPT);
        assert_fits(name, std::mem::take(&mut self.overflows));
    }

    fn note_overflows(&mut self) {
        let found: Vec<String> = self
            .document()
            .overflows()
            .iter()
            .map(ToString::to_string)
            .collect();
        self.overflows.extend(found);
    }

    fn painted(&mut self) -> Snapshot {
        let behind = beui::styled::Theme::of(self.document().preferences().theme).background;
        let shown = self
            .shown_output()
            .expect("the editor has not drawn its window yet");
        let painting = beui::native::painting::painting(
            shown,
            self.size,
            shown.pixels_per_point,
            behind,
            beui::native::painting::Punches::Shown,
        )
        .and_then(Snapshot::of_painting);
        painting.expect("the painting could not be rendered")
    }
}

fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned>(value: T, what: &str) -> T {
    be_sdk_api::ops::decode(&be_sdk_api::ops::encode(&value))
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
        sent: &mut Vec<ShellMessage>,
        region: Rect,
        origin: Pos2,
        intrinsic: Option<Vec2>,
        drawn: bool,
    ) {
        let mut changes = Vec::new();
        sent.retain(|call| match call {
            ShellMessage::ChangeView(change) => {
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

fn acknowledges(frame: &[u8], acknowledged: &mut std::collections::HashSet<OpId>) -> bool {
    match be_sdk_api::ops::decode_frame(frame) {
        Ok(be_sdk_api::ops::Frame::Complete { op, outcome }) => {
            acknowledged.remove(&op) || outcome == Outcome::Err(OpError::Unanswered)
        }
        _ => false,
    }
}

fn extension_call(method: be_sdk_api::ops::MethodId) -> bool {
    be_sdk_api::embed::method_name(method)
        .and_then(|name| name.split_once('#'))
        .is_some_and(|(interface, _)| be_sdk_api::is_published(interface))
}

fn published(method: be_sdk_api::ops::MethodId) -> Option<be_sdk_api::ops::Watching> {
    be_sdk_api::embed::WATCHES
        .iter()
        .find(|watching| {
            (watching.open == method || watching.next == method)
                && be_sdk_api::is_published(watching.interface)
        })
        .copied()
}

fn available_status(view: ViewId, placement: &ChildPlacement) -> ChildStatus {
    ChildStatus {
        instance: INSTANCE,
        view,
        child: placement.child,
        available: true,
        intrinsic: None,
        active: false,
        interaction: be_sdk_beui::InteractionMode::Preview,
        capabilities: be_sdk_beui::EditorCapabilities::default(),
        resize: be_sdk_beui::ResizeMode::None,
        error: None,
        menu: Vec::new(),
        creation: None,
        preview: Default::default(),
        dialog: Default::default(),
        picked: None,
        card: false,
        title: None,
        icon: None,
        group: None,
        raised: 0,
        rung: 0,
        minimized: 0,
        fullscreen: false,
        notice: None,
        menu_asked: None,
    }
}
