use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::graph::BlockParent;
use block_plugin_api::TopBar;
use block_plugin_api::{
    AccessLevel, AccessListing, ArtifactAction, AudioCommand, AudioStatus, BarAction, BlockCommand,
    BlockPick, ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement, ChildRect,
    ChildStatus, ClipboardImage, DataListing, EditorRegion, FetchResult, FilePick, FileSave,
    HostDisplay, HostInputDevice, HostPanel, HostReply, HostRequest, HostWindow, HostWindowId,
    HostNotification, LinuxMessage, MenuEntry, Occluder, PerformanceMeasurement, PowerAction,
    PowerAvailability, ShellDialog, Size,
    ViewChange, WebViewCommand, WebViewEvent, WebViewId,
};
pub use block_plugin_api::{BlockFilter, FileFilter, SavedFile};
use block_ui::BlockCatalog;
use geometry::{Pos2, Rect, Vec2, vec2};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct BlockDrag {
    pub position: Pos2,
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub dropped: bool,
}

#[derive(Clone)]
pub struct HostContent {
    pub content_type: Uuid,
    pub bytes: Vec<u8>,
    pub session: Vec<u8>,
    pub applied: u64,
}

impl HostContent {
    pub fn of<C: be_block::LiveEdit>(content: &C) -> Self {
        Self {
            content_type: C::CONTENT_TYPE,
            bytes: content.encode(),
            session: content.session_state(),
            applied: 0,
        }
    }
}

type ContentOperation = (Option<Uuid>, Vec<u8>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShownPresence {
    pub block: Option<Uuid>,
    pub kind: Uuid,
    pub value: Option<Vec<u8>>,
}

type Peers = (u64, Vec<PeerPresence>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerPresence {
    pub client: u64,
    pub kind: Uuid,
    pub value: Vec<u8>,
}

#[derive(Clone)]
pub enum ContentUpdate {
    Snapshot(HostContent),
    Operations(Vec<(Vec<u8>, bool)>),
}

#[derive(Clone, PartialEq)]
pub struct FileDrop {
    pub position: Pos2,
    pub files: Vec<PickedFile>,
    pub dropped: bool,
}

pub type OpenRequest = (Uuid, Uuid, Option<Uuid>);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChildCommit {
    pub(crate) child: ChildId,
    pub(crate) parent: BlockParent,
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickRequest {
    pub pick: u64,
    pub filter: BlockFilter,
    pub parent: BlockParent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShowRequest {
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub via: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockHistory {
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArtifactState {
    pub block_id: Uuid,
    pub source_type: Uuid,
    pub source: Option<Uuid>,
    pub summary: String,
    pub error: Option<String>,
    pub regenerating: bool,
}

#[derive(Clone, Copy)]
pub struct PickedBlock {
    pub id: Uuid,
    pub block_type: Uuid,
    pub linked: bool,
    pub placed: bool,
}

#[derive(Clone, Default, PartialEq)]
pub struct FocusedBlock {
    pub block_id: Option<Uuid>,
    pub block_type: Uuid,
    pub via: Vec<Uuid>,
}

impl FocusedBlock {
    fn same(&self, other: &Self) -> bool {
        self.block_id == other.block_id
            && self.block_type == other.block_type
            && self.via == other.via
    }
}

#[derive(Clone, Copy)]
pub struct Artifact {
    pub block_id: Uuid,
    pub block_type: Uuid,
}

pub struct ArtifactDescription {
    pub source: Uuid,
    pub summary: String,
}

#[derive(Clone, PartialEq)]
pub struct PickedFile {
    pub name: String,
    pub data: Vec<u8>,
}

type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Default)]
pub struct Waker(Arc<OnceLock<Wake>>);

impl Waker {
    pub fn wake(&self) {
        if let Some(wake) = self.0.get() {
            wake();
        }
    }

    pub fn install(&self, wake: impl Fn() + Send + Sync + 'static) -> bool {
        self.0.set(Arc::new(wake)).is_ok()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let waker = Self::default();
        waker.install(wake);
        waker
    }

    pub fn counting(&self, count: Arc<std::sync::atomic::AtomicU64>) -> Self {
        let inner = self.clone();
        let waker = Self::default();
        waker.install(move || {
            count.fetch_add(1, std::sync::atomic::Ordering::Release);
            inner.wake();
        });
        waker
    }
}

struct PerformanceRecord {
    group: Arc<str>,
    measurement: PerformanceMeasurement,
}

#[derive(Clone)]
pub struct PerformanceReporter {
    group: Arc<str>,
    records: Arc<Mutex<Vec<PerformanceRecord>>>,
}

impl PerformanceReporter {
    pub fn measure(&self, name: impl Into<String>) -> PerformanceMeasurementGuard {
        PerformanceMeasurementGuard {
            reporter: self.clone(),
            name: name.into(),
            started: Instant::now(),
        }
    }

    pub fn record_duration(&self, name: impl Into<String>, duration: Duration) {
        self.record(PerformanceMeasurement::Duration {
            name: name.into(),
            nanoseconds: duration.as_nanos().min(u128::from(u64::MAX)) as u64,
        });
    }

    pub fn record_count(&self, name: impl Into<String>, count: u64) {
        self.record(PerformanceMeasurement::Count {
            name: name.into(),
            count,
        });
    }

    fn record(&self, measurement: PerformanceMeasurement) {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(PerformanceRecord {
                group: Arc::clone(&self.group),
                measurement,
            });
    }
}

pub struct PerformanceMeasurementGuard {
    reporter: PerformanceReporter,
    name: String,
    started: Instant,
}

impl Drop for PerformanceMeasurementGuard {
    fn drop(&mut self) {
        self.reporter
            .record_duration(std::mem::take(&mut self.name), self.started.elapsed());
    }
}

#[derive(Clone, Copy, Default)]
struct Region {
    region: Option<EditorRegion>,
    origin: Vec2,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum Identity {
    Block([u8; 16]),
    WebView(WebViewId),
    Host(HostPanel),
    Window(HostWindowId),
    Creation([u8; 16], String),
    ArtifactSettings([u8; 16]),
}

impl Identity {
    fn of(content: &ChildContent) -> Self {
        match content {
            ChildContent::Block { block_id, .. } => Self::Block(*block_id),
            ChildContent::WebView(web_view) => Self::WebView(*web_view),
            ChildContent::Host(panel) => Self::Host(*panel),
            ChildContent::Window(window) => Self::Window(*window),
            ChildContent::Creation { editor, template } => {
                Self::Creation(*editor, template.clone())
            }
            ChildContent::ArtifactSettings { block_id } => Self::ArtifactSettings(*block_id),
        }
    }
}

type ChildKey = (EditorRegion, Identity, u32);

#[derive(Default)]
struct Children {
    placements: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    ordinals: HashMap<Identity, u32>,
    identities: HashMap<ChildKey, ChildId>,
    used: Vec<ChildKey>,
    next: u64,
}

impl Children {
    fn identify(&mut self, region: EditorRegion, identity: Identity) -> ChildId {
        let ordinal = {
            let ordinal = self.ordinals.entry(identity.clone()).or_default();
            let current = *ordinal;
            *ordinal += 1;
            current
        };
        let key = (region, identity, ordinal);
        let child = match self.identities.get(&key) {
            Some(child) => *child,
            None => {
                self.next += 1;
                let child = ChildId(self.next);
                self.identities.insert(key.clone(), child);
                child
            }
        };
        self.used.push(key);
        child
    }
}

#[derive(Clone, Copy)]
struct View {
    rect: Rect,
    scale: f32,
}

fn swept(rect: Rect, rotation: f32) -> Rect {
    if rotation == 0.0 {
        return rect;
    }
    let center = rect.center();
    let (sin, cos) = rotation.sin_cos();
    let turned = |corner: Pos2| {
        let offset = corner - center;
        center
            + vec2(
                offset.x * cos - offset.y * sin,
                offset.x * sin + offset.y * cos,
            )
    };
    Rect::from_points(&[
        turned(rect.left_top()),
        turned(rect.right_top()),
        turned(rect.right_bottom()),
        turned(rect.left_bottom()),
    ])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pushed {
    Replies,
    Peers,
    Histories,
    Artifacts,
    Audio,
    Focus,
    Catalog,
    WebView,
    Shows,
    Version,
    Windows,
    InputDevices,
    Displays,
    Power,
    Notifications,
}

impl Pushed {
    pub const ALL: [Self; 15] = [
        Self::Replies,
        Self::Peers,
        Self::Histories,
        Self::Artifacts,
        Self::Audio,
        Self::Focus,
        Self::Catalog,
        Self::WebView,
        Self::Shows,
        Self::Version,
        Self::Windows,
        Self::InputDevices,
        Self::Displays,
        Self::Power,
        Self::Notifications,
    ];
}

#[derive(Clone, Default)]
pub struct EditorHost {
    waker: Waker,
    shown_panels: Rc<RefCell<Vec<HostPanel>>>,
    windows: Rc<RefCell<Vec<HostWindow>>>,
    input_devices: Rc<RefCell<Vec<HostInputDevice>>>,
    watching_input_devices: Rc<Cell<bool>>,
    reported_input_device_watch: Rc<Cell<bool>>,
    displays: Rc<RefCell<Vec<HostDisplay>>>,
    watching_displays: Rc<Cell<bool>>,
    reported_display_watch: Rc<Cell<bool>>,
    power: Rc<Cell<PowerAvailability>>,
    watching_power: Rc<Cell<bool>>,
    reported_power_watch: Rc<Cell<bool>>,
    power_requests: Rc<RefCell<Vec<PowerAction>>>,
    notifications: Rc<RefCell<Vec<HostNotification>>>,
    watching_notifications: Rc<Cell<bool>>,
    reported_notification_watch: Rc<Cell<bool>>,
    notification_requests: Rc<RefCell<Vec<LinuxMessage>>>,
    closed_windows: Rc<RefCell<Vec<HostWindowId>>>,
    fullscreen_windows: Rc<RefCell<Vec<(HostWindowId, bool)>>>,
    focused_windows: Rc<RefCell<Vec<HostWindowId>>>,
    pick_requests: Rc<RefCell<Vec<PickRequest>>>,
    dialog_requests: Rc<RefCell<Vec<(Uuid, ShellDialog)>>>,
    access_changes: Rc<RefCell<Vec<(Uuid, Uuid, AccessLevel)>>>,
    pick_answers: Rc<RefCell<Vec<(u64, BlockPick)>>>,
    child_commits: Rc<RefCell<Vec<ChildCommit>>>,
    pushed: Rc<[Cell<u64>; Pushed::ALL.len()]>,
    changes: Rc<Cell<u64>>,
    opens: Rc<RefCell<Vec<OpenRequest>>>,
    shows: Rc<RefCell<Vec<ShowRequest>>>,
    focused: Rc<RefCell<FocusedBlock>>,
    reported_focus: Rc<RefCell<Option<FocusedBlock>>>,
    artifacts: Rc<RefCell<HashMap<Uuid, ArtifactState>>>,
    watched_artifacts: Rc<RefCell<Vec<Uuid>>>,
    reported_artifacts: Rc<RefCell<Option<Vec<Uuid>>>>,
    histories: Rc<RefCell<HashMap<Uuid, BlockHistory>>>,
    watched_history: Rc<RefCell<Vec<Uuid>>>,
    reported_history: Rc<RefCell<Option<Vec<Uuid>>>>,
    block_drags: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
    block_commands: Rc<RefCell<Vec<(Uuid, BlockCommand)>>>,
    version_commands: Rc<RefCell<Vec<(Uuid, block_plugin_api::VersionCommand)>>>,
    version_status: Rc<RefCell<block_plugin_api::VersionStatus>>,
    block_types: Rc<RefCell<Rc<BlockCatalog>>>,
    drag: Rc<Cell<Option<BlockDrag>>>,
    files: Rc<RefCell<Option<FileDrop>>>,
    drag_accepted: Rc<Cell<Option<bool>>>,
    requests: Rc<RefCell<Vec<(u64, HostRequest)>>>,
    replies: Rc<RefCell<HashMap<u64, HostReply>>>,
    next_request: Rc<Cell<u64>>,
    editable: Rc<Cell<bool>>,
    block_type: Rc<Cell<Option<Uuid>>>,
    client_id: Rc<Cell<Uuid>>,
    view_block: Rc<Cell<Option<Uuid>>>,
    view: Rc<Cell<Option<View>>>,
    view_changes: Rc<RefCell<Vec<ViewChange>>>,
    creation_ready: Rc<Cell<bool>>,
    creation_changed: Rc<Cell<bool>>,
    performance: Arc<Mutex<Vec<PerformanceRecord>>>,
    region: Rc<Cell<Region>>,
    children: Rc<RefCell<Children>>,
    child_statuses: Rc<RefCell<HashMap<ChildId, ChildStatus>>>,
    audio_commands: Rc<RefCell<Vec<(Uuid, AudioCommand)>>>,
    audio_status: Rc<RefCell<AudioStatus>>,
    web_view_commands: Rc<RefCell<Vec<(WebViewId, WebViewCommand)>>>,
    web_view_events: Rc<RefCell<Vec<(WebViewId, WebViewEvent)>>>,
    cursor_grabbed: Rc<Cell<bool>>,
    cursor_grab_changed: Rc<Cell<bool>>,
    presenting: Rc<Cell<bool>>,
    present_requests: Rc<RefCell<Vec<bool>>>,
    child_views: Rc<RefCell<HashMap<ChildId, Vec<ViewChange>>>>,
    child_bars: Rc<RefCell<HashMap<ChildId, Vec<BarAction>>>>,
    bar_actions: Rc<RefCell<Vec<BarAction>>>,
    menu: Rc<RefCell<Vec<MenuEntry>>>,
    menu_picks: Rc<RefCell<Vec<String>>>,
    child_menu_picks: Rc<RefCell<Vec<(ChildId, String)>>>,
    chrome: Rc<Cell<Option<bool>>>,
    content: Rc<Cell<Option<Rect>>>,
    copied: Rc<RefCell<Vec<String>>>,
    paste_requested: Rc<Cell<bool>>,
    leaving: Rc<Cell<bool>>,
    next_frame: Rc<Cell<Option<Duration>>>,
    content_updates: Rc<RefCell<HashMap<Option<Uuid>, Vec<ContentUpdate>>>>,
    content_operations: Rc<RefCell<Vec<ContentOperation>>>,
    resend_requests: Rc<RefCell<Vec<Option<Uuid>>>>,
    watched_content: Rc<RefCell<std::collections::BTreeMap<Uuid, Uuid>>>,
    seeded: Rc<RefCell<Vec<SeededContent>>>,
    shown: Rc<RefCell<Vec<ShownPresence>>>,
    peers: Rc<RefCell<HashMap<Option<Uuid>, Peers>>>,
    next_peers: Rc<Cell<u64>>,
    reported_content: Rc<RefCell<Option<std::collections::BTreeMap<Uuid, Uuid>>>>,
    graph: Rc<crate::graph::GraphState>,
    account: Rc<Cell<Uuid>>,
    workspace: Rc<Cell<Uuid>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeededContent {
    pub block: Uuid,
    pub content_type: Uuid,
    pub bytes: Vec<u8>,
    pub replace: bool,
}

impl EditorHost {
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            waker,
            ..Self::default()
        }
    }

    pub fn waker(&self) -> Waker {
        self.waker.clone()
    }

    pub fn revision(&self, pushed: Pushed) -> u64 {
        self.pushed[pushed as usize].get()
    }

    fn push(&self, pushed: Pushed) {
        let revision = &self.pushed[pushed as usize];
        revision.set(revision.get() + 1);
        self.changed();
    }

    pub fn changes(&self) -> u64 {
        self.changes.get() + self.graph.deferred()
    }

    fn changed(&self) {
        self.changes.set(self.changes.get() + 1);
    }

    pub fn performance(&self, group: impl Into<String>) -> PerformanceReporter {
        PerformanceReporter {
            group: Arc::from(group.into()),
            records: Arc::clone(&self.performance),
        }
    }

    pub fn open_block(&self, block_id: Uuid, block_type: Uuid) {
        self.opens.borrow_mut().push((block_id, block_type, None));
    }

    pub fn open_block_via(&self, block_id: Uuid, block_type: Uuid, container: Uuid) {
        self.opens
            .borrow_mut()
            .push((block_id, block_type, Some(container)));
    }

    pub fn take_show_requests(&self) -> Vec<ShowRequest> {
        std::mem::take(&mut self.shows.borrow_mut())
    }

    pub fn take_dialog_requests(&self) -> Vec<(Uuid, ShellDialog)> {
        std::mem::take(&mut self.dialog_requests.borrow_mut())
    }

    pub fn show_dialog(&self, block_id: Uuid, dialog: ShellDialog) {
        self.dialog_requests.borrow_mut().push((block_id, dialog));
        self.push(Pushed::Shows);
    }

    pub fn take_pick_requests(&self) -> Vec<PickRequest> {
        std::mem::take(&mut self.pick_requests.borrow_mut())
    }

    pub fn request_pick(&self, request: PickRequest) {
        self.pick_requests.borrow_mut().push(request);
        self.push(Pushed::Shows);
    }

    pub fn answer_pick(&self, pick: u64, answer: BlockPick) {
        self.pick_answers.borrow_mut().push((pick, answer));
        self.changed();
    }

    pub(crate) fn take_pick_answers(&self) -> Vec<(u64, BlockPick)> {
        std::mem::take(&mut self.pick_answers.borrow_mut())
    }

    pub fn commit_child(&self, child: ChildId, parent: BlockParent, name: Option<String>) {
        self.child_commits.borrow_mut().push(ChildCommit {
            child,
            parent,
            name,
        });
        self.changed();
    }

    pub(crate) fn take_child_commits(&self) -> Vec<ChildCommit> {
        std::mem::take(&mut self.child_commits.borrow_mut())
    }

    pub fn take_panel_requests(&self) -> Vec<HostPanel> {
        std::mem::take(&mut self.shown_panels.borrow_mut())
    }

    pub fn show_panel(&self, panel: HostPanel) {
        self.shown_panels.borrow_mut().push(panel);
        self.push(Pushed::Shows);
    }

    pub fn windows(&self) -> Vec<HostWindow> {
        self.windows.borrow().clone()
    }

    pub fn set_windows(&self, windows: Vec<HostWindow>) {
        if *self.windows.borrow() == windows {
            return;
        }
        *self.windows.borrow_mut() = windows;
        self.push(Pushed::Windows);
    }

    pub fn input_devices(&self) -> Vec<HostInputDevice> {
        self.watching_input_devices.set(true);
        self.input_devices.borrow().clone()
    }

    pub fn set_input_devices(&self, devices: Vec<HostInputDevice>) {
        if *self.input_devices.borrow() == devices {
            return;
        }
        *self.input_devices.borrow_mut() = devices;
        self.push(Pushed::InputDevices);
    }

    pub fn displays(&self) -> Vec<HostDisplay> {
        self.watching_displays.set(true);
        self.displays.borrow().clone()
    }

    pub fn set_displays(&self, displays: Vec<HostDisplay>) {
        if *self.displays.borrow() == displays {
            return;
        }
        *self.displays.borrow_mut() = displays;
        self.push(Pushed::Displays);
    }

    pub(crate) fn take_display_watch(&self) -> bool {
        let wanted = self.watching_displays.get() && !self.reported_display_watch.get();
        if wanted {
            self.reported_display_watch.set(true);
        }
        wanted
    }

    pub fn power(&self) -> PowerAvailability {
        self.watching_power.set(true);
        self.power.get()
    }

    pub fn set_power(&self, power: PowerAvailability) {
        if self.power.get() == power {
            return;
        }
        self.power.set(power);
        self.push(Pushed::Power);
    }

    pub(crate) fn take_power_watch(&self) -> bool {
        let wanted = self.watching_power.get() && !self.reported_power_watch.get();
        if wanted {
            self.reported_power_watch.set(true);
        }
        wanted
    }

    pub fn request_power(&self, action: PowerAction) {
        self.power_requests.borrow_mut().push(action);
        self.changed();
    }

    pub(crate) fn take_power_requests(&self) -> Vec<PowerAction> {
        std::mem::take(&mut self.power_requests.borrow_mut())
    }

    pub fn notifications(&self) -> Vec<HostNotification> {
        self.watching_notifications.set(true);
        self.notifications.borrow().clone()
    }

    pub fn set_notifications(&self, notifications: Vec<HostNotification>) {
        if *self.notifications.borrow() == notifications {
            return;
        }
        *self.notifications.borrow_mut() = notifications;
        self.push(Pushed::Notifications);
    }

    pub(crate) fn take_notification_watch(&self) -> bool {
        let wanted =
            self.watching_notifications.get() && !self.reported_notification_watch.get();
        if wanted {
            self.reported_notification_watch.set(true);
        }
        wanted
    }

    pub fn invoke_notification(&self, id: u32, action: String) {
        self.notification_requests
            .borrow_mut()
            .push(LinuxMessage::InvokeNotification { id, action });
        self.changed();
    }

    pub fn dismiss_notifications(&self, ids: Vec<u32>) {
        if ids.is_empty() {
            return;
        }
        self.notification_requests
            .borrow_mut()
            .push(LinuxMessage::DismissNotifications(ids));
        self.changed();
    }

    pub(crate) fn take_notification_requests(&self) -> Vec<LinuxMessage> {
        std::mem::take(&mut self.notification_requests.borrow_mut())
    }

    pub(crate) fn take_input_device_watch(&self) -> bool {
        let wanted = self.watching_input_devices.get() && !self.reported_input_device_watch.get();
        if wanted {
            self.reported_input_device_watch.set(true);
        }
        wanted
    }

    pub fn close_window(&self, window: HostWindowId) {
        self.closed_windows.borrow_mut().push(window);
        self.changed();
    }

    pub(crate) fn take_closed_windows(&self) -> Vec<HostWindowId> {
        std::mem::take(&mut self.closed_windows.borrow_mut())
    }

    pub fn fullscreen_window(&self, window: HostWindowId, fullscreen: bool) {
        self.fullscreen_windows
            .borrow_mut()
            .push((window, fullscreen));
        self.changed();
    }

    pub(crate) fn take_fullscreen_windows(&self) -> Vec<(HostWindowId, bool)> {
        std::mem::take(&mut self.fullscreen_windows.borrow_mut())
    }

    pub fn focus_window(&self, window: HostWindowId) {
        self.focused_windows.borrow_mut().push(window);
        self.changed();
    }

    pub(crate) fn take_focused_windows(&self) -> Vec<HostWindowId> {
        std::mem::take(&mut self.focused_windows.borrow_mut())
    }

    pub fn show_block(&self, block_id: Uuid, block_type: Uuid, via: Option<Uuid>) {
        self.shows.borrow_mut().push(ShowRequest {
            block_id,
            block_type,
            via,
        });
        self.push(Pushed::Shows);
    }

    pub fn focused_block(&self) -> FocusedBlock {
        self.focused.borrow().clone()
    }

    pub fn report_focus(&self, focused: FocusedBlock) {
        if self.focused.borrow().same(&focused) {
            return;
        }
        *self.focused.borrow_mut() = focused;
        self.push(Pushed::Focus);
        self.waker.wake();
    }

    pub(crate) fn take_focus_report(&self) -> Option<FocusedBlock> {
        let focused = self.focused.borrow().clone();
        let mut reported = self.reported_focus.borrow_mut();
        if reported.as_ref().is_some_and(|last| last.same(&focused)) {
            return None;
        }
        *reported = Some(focused.clone());
        Some(focused)
    }

    pub fn watch_artifacts(&self, blocks: impl IntoIterator<Item = Uuid>) {
        let mut blocks: Vec<Uuid> = blocks.into_iter().collect();
        blocks.sort();
        blocks.dedup();
        *self.watched_artifacts.borrow_mut() = blocks;
    }

    pub fn artifact(&self, block_id: Uuid) -> Option<ArtifactState> {
        self.artifacts.borrow().get(&block_id).cloned()
    }

    pub(crate) fn take_artifact_watch(&self) -> Option<Vec<Uuid>> {
        let blocks = self.watched_artifacts.borrow().clone();
        let mut reported = self.reported_artifacts.borrow_mut();
        if reported.as_ref() == Some(&blocks) {
            return None;
        }
        *reported = Some(blocks.clone());
        Some(blocks)
    }

    pub fn watch_history(&self, blocks: impl IntoIterator<Item = Uuid>) {
        let mut blocks: Vec<Uuid> = blocks.into_iter().collect();
        blocks.sort();
        blocks.dedup();
        *self.watched_history.borrow_mut() = blocks;
    }

    pub fn history(&self, block_id: Uuid) -> BlockHistory {
        self.histories
            .borrow()
            .get(&block_id)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn take_history_watch(&self) -> Option<Vec<Uuid>> {
        let blocks = self.watched_history.borrow().clone();
        let mut reported = self.reported_history.borrow_mut();
        if reported.as_ref() == Some(&blocks) {
            return None;
        }
        *reported = Some(blocks.clone());
        Some(blocks)
    }

    pub fn set_histories(&self, states: impl IntoIterator<Item = (Uuid, BlockHistory)>) {
        *self.histories.borrow_mut() = states.into_iter().collect();
        self.push(Pushed::Histories);
    }

    pub fn undo(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::Undo));
    }

    pub fn redo(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::Redo));
    }

    pub fn set_artifacts(&self, states: Vec<ArtifactState>) {
        *self.artifacts.borrow_mut() = states
            .into_iter()
            .map(|state| (state.block_id, state))
            .collect();
        self.push(Pushed::Artifacts);
    }

    pub fn regenerate_artifact(&self, block_id: Uuid) {
        self.artifact_command(block_id, ArtifactAction::Regenerate);
    }

    pub fn unlink_artifact(&self, block_id: Uuid) {
        self.artifact_command(block_id, ArtifactAction::Unlink);
    }

    fn artifact_command(&self, block_id: Uuid, action: ArtifactAction) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::Artifact { action }));
    }

    pub fn close_editor(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::CloseEditor));
    }

    pub fn simulate_access(&self, block_id: Uuid, access: AccessLevel) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::SimulateAccess { access }));
    }

    pub(crate) fn set_focused_block(&self, focused: FocusedBlock) {
        *self.focused.borrow_mut() = focused;
        self.push(Pushed::Focus);
    }

    pub fn drag_block(&self, block_id: Uuid, block_type: Uuid) {
        self.block_drags.borrow_mut().push((block_id, block_type));
    }

    pub(crate) fn take_block_drags(&self) -> Vec<(Uuid, Uuid)> {
        std::mem::take(&mut self.block_drags.borrow_mut())
    }

    pub fn share_block(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::Share));
    }

    pub fn show_app_menu(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::AppMenu));
    }

    pub fn show_launcher(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::Launcher));
    }

    pub fn rename_block(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::Rename));
    }

    pub fn unlink_block(&self, block_id: Uuid, container: Uuid) {
        self.block_commands.borrow_mut().push((
            block_id,
            BlockCommand::Unlink {
                container: container.into_bytes(),
            },
        ));
    }

    pub fn delete_block(
        &self,
        block_id: Uuid,
        block_type: Uuid,
        source: BlockParent,
        is_reference: bool,
    ) {
        self.block_commands.borrow_mut().push((
            block_id,
            BlockCommand::Delete {
                block_type: block_type.into_bytes(),
                source: source.encode(),
                is_reference,
            },
        ));
    }

    pub fn move_block(
        &self,
        block_id: Uuid,
        block_type: Uuid,
        source: BlockParent,
        destination: Uuid,
        is_reference: bool,
    ) {
        self.block_commands.borrow_mut().push((
            block_id,
            BlockCommand::Move {
                block_type: block_type.into_bytes(),
                source: source.encode(),
                destination: destination.into_bytes(),
                is_reference,
            },
        ));
    }

    pub fn place_block(&self, block_id: Uuid, block_type: Uuid, parent: Uuid, linked: bool) {
        self.block_commands.borrow_mut().push((
            block_id,
            BlockCommand::Place {
                block_type: block_type.into_bytes(),
                parent: parent.into_bytes(),
                linked,
            },
        ));
    }

    pub fn version(&self, block_id: Uuid, command: block_plugin_api::VersionCommand) {
        self.version_commands.borrow_mut().push((block_id, command));
    }

    pub fn take_version_commands(&self) -> Vec<(Uuid, block_plugin_api::VersionCommand)> {
        std::mem::take(&mut self.version_commands.borrow_mut())
    }

    pub fn set_version_status(&self, status: block_plugin_api::VersionStatus) {
        *self.version_status.borrow_mut() = status;
        self.push(Pushed::Version);
    }

    pub fn version_status(&self) -> block_plugin_api::VersionStatus {
        self.version_status.borrow().clone()
    }

    pub fn take_block_commands(&self) -> Vec<(Uuid, BlockCommand)> {
        std::mem::take(&mut self.block_commands.borrow_mut())
    }

    pub fn block_types(&self) -> Rc<BlockCatalog> {
        Rc::clone(&self.block_types.borrow())
    }

    pub fn editable(&self) -> bool {
        self.editable.get()
    }

    pub fn block_type(&self) -> Option<Uuid> {
        self.block_type.get()
    }

    pub fn set_block_type(&self, block_type: Uuid) {
        self.block_type.set(Some(block_type));
    }

    pub fn set_block_content(&self, content: HostContent) {
        self.update_content(None, ContentUpdate::Snapshot(content));
    }

    pub fn set_content_of(&self, block: Uuid, content: HostContent) {
        self.update_content(Some(block), ContentUpdate::Snapshot(content));
    }

    pub fn push_content_operations(&self, operations: Vec<(Vec<u8>, bool)>) {
        self.update_content(None, ContentUpdate::Operations(operations));
    }

    pub fn push_content_operations_of(&self, block: Uuid, operations: Vec<(Vec<u8>, bool)>) {
        self.update_content(Some(block), ContentUpdate::Operations(operations));
    }

    fn update_content(&self, block: Option<Uuid>, update: ContentUpdate) {
        self.content_updates
            .borrow_mut()
            .entry(block)
            .or_default()
            .push(update);
        self.changed();
    }

    pub fn updated_content(&self) -> Vec<Option<Uuid>> {
        self.content_updates.borrow().keys().copied().collect()
    }

    pub(crate) fn take_content_updates(&self, block: Option<Uuid>) -> Vec<ContentUpdate> {
        self.content_updates
            .borrow_mut()
            .remove(&block)
            .unwrap_or_default()
    }

    pub fn operate_content(&self, operation: Vec<u8>) {
        self.operate_content_at(None, operation);
    }

    pub(crate) fn operate_content_at(&self, block: Option<Uuid>, operation: Vec<u8>) {
        self.content_operations
            .borrow_mut()
            .push((block, operation));
    }

    pub fn take_content_operations(&self) -> Vec<Vec<u8>> {
        self.take_operations_where(None)
    }

    pub fn take_content_operations_of(&self, block: Uuid) -> Vec<Vec<u8>> {
        self.take_operations_where(Some(block))
    }

    fn take_operations_where(&self, block: Option<Uuid>) -> Vec<Vec<u8>> {
        let mut held = self.content_operations.borrow_mut();
        let (taken, kept) = std::mem::take(&mut *held)
            .into_iter()
            .partition::<Vec<_>, _>(|(target, _)| *target == block);
        *held = kept;
        taken.into_iter().map(|(_, operation)| operation).collect()
    }

    pub(crate) fn take_all_content_operations(&self) -> Vec<(Option<Uuid>, Vec<u8>)> {
        std::mem::take(&mut self.content_operations.borrow_mut())
    }

    pub(crate) fn request_content_resend(&self, block: Option<Uuid>) {
        let mut requests = self.resend_requests.borrow_mut();
        if !requests.contains(&block) {
            requests.push(block);
        }
        drop(requests);
        self.waker.wake();
    }

    pub(crate) fn take_content_resend_requests(&self) -> Vec<Option<Uuid>> {
        std::mem::take(&mut self.resend_requests.borrow_mut())
    }

    pub fn watch_content(&self, block: Uuid, content_type: Uuid) {
        self.watched_content
            .borrow_mut()
            .insert(block, content_type);
    }

    pub fn content_of<C>(&self, block: Uuid) -> crate::ContentProjection<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        self.watch_content(block, C::CONTENT_TYPE);
        crate::ContentProjection::new(self.clone(), Some(block))
    }

    pub fn seed_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.write_content(block, content, false);
    }

    pub fn replace_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.write_content(block, content, true);
    }

    fn write_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C, replace: bool) {
        self.seed_bytes(block, C::CONTENT_TYPE, content.encode(), replace);
    }

    pub(crate) fn seed_bytes(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
        replace: bool,
    ) {
        self.seeded.borrow_mut().push(SeededContent {
            block,
            content_type,
            bytes,
            replace,
        });
        self.waker.wake();
    }

    pub fn show_presence(&self, block: Option<Uuid>, kind: Uuid, value: Option<Vec<u8>>) {
        let mut shown = self.shown.borrow_mut();
        shown.retain(|held| held.block != block || held.kind != kind);
        shown.push(ShownPresence { block, kind, value });
        self.waker.wake();
    }

    pub fn take_shown_presence(&self) -> Vec<ShownPresence> {
        std::mem::take(&mut self.shown.borrow_mut())
    }

    pub fn set_peers(&self, block: Option<Uuid>, peers: Vec<PeerPresence>) {
        let revision = self.next_peers.get() + 1;
        self.next_peers.set(revision);
        self.peers.borrow_mut().insert(block, (revision, peers));
        self.push(Pushed::Peers);
        self.waker.wake();
    }

    pub fn peers_since(&self, block: Option<Uuid>, seen: u64) -> Option<(u64, Vec<PeerPresence>)> {
        let peers = self.peers.borrow();
        let (revision, held) = peers.get(&block)?;
        (*revision != seen).then(|| (*revision, held.clone()))
    }

    pub fn take_seeded_content(&self) -> Vec<SeededContent> {
        std::mem::take(&mut self.seeded.borrow_mut())
    }

    pub fn watched_content(&self) -> Vec<(Uuid, Uuid)> {
        self.watched_content
            .borrow()
            .iter()
            .map(|(block, content_type)| (*block, *content_type))
            .collect()
    }

    pub(crate) fn take_content_watch(&self) -> Option<Vec<(Uuid, Uuid)>> {
        let watched = self.watched_content.borrow().clone();
        let mut reported = self.reported_content.borrow_mut();
        if reported.as_ref() == Some(&watched) {
            return None;
        }
        *reported = Some(watched.clone());
        Some(watched.into_iter().collect())
    }

    pub fn client_id(&self) -> Uuid {
        self.client_id.get()
    }

    pub fn view_block(&self) -> Option<Uuid> {
        self.view_block.get()
    }

    pub fn set_view_block(&self, view_block: Option<Uuid>) {
        self.view_block.set(view_block);
    }

    pub fn view(&self) -> Option<Rect> {
        let origin = self.region.get().origin;
        self.view.get().map(|view| view.rect.translate(origin))
    }

    pub fn view_scale(&self) -> Option<f32> {
        self.view.get().map(|view| view.scale)
    }

    pub fn pan_view(&self, delta: Vec2) {
        self.view_changes.borrow_mut().push(ViewChange::Pan {
            x: delta.x,
            y: delta.y,
        });
    }

    pub fn zoom_view(&self, factor: f32, anchor: Option<Pos2>) {
        let origin = self.region.get().origin;
        self.view_changes.borrow_mut().push(ViewChange::Zoom {
            factor,
            anchor: anchor.map(|anchor| (anchor.x - origin.x, anchor.y - origin.y)),
        });
    }

    pub fn fit_view(&self) {
        self.view_changes.borrow_mut().push(ViewChange::Fit);
    }

    pub fn resume_auto_fit_view(&self) {
        self.view_changes
            .borrow_mut()
            .push(ViewChange::ResumeAutoFit);
    }

    pub fn drag(&self) -> Option<BlockDrag> {
        self.drag.get()
    }

    pub fn files(&self) -> Option<FileDrop> {
        self.files.borrow().clone()
    }

    pub(crate) fn set_files(&self, drop: Option<FileDrop>) {
        *self.files.borrow_mut() = drop;
        self.changed();
    }

    pub fn accept_drag(&self, accepted: bool) {
        self.drag_accepted.set(Some(accepted));
    }

    fn ask(&self, request: HostRequest) -> u64 {
        let id = self.next_request.get() + 1;
        self.next_request.set(id);
        self.requests.borrow_mut().push((id, request));
        id
    }

    pub fn pick_file(&self, filter: FileFilter) -> u64 {
        self.ask(HostRequest::PickFile(filter))
    }

    pub fn take_pick(&self, request: u64) -> Option<FilePick> {
        match self.take_reply(request)? {
            HostReply::FilePicked(pick) => Some(pick),
            reply => self.mismatched(request, reply),
        }
    }

    pub fn save_file(&self, file: SavedFile) -> u64 {
        self.ask(HostRequest::SaveFile(file))
    }

    pub fn take_save(&self, request: u64) -> Option<FileSave> {
        match self.take_reply(request)? {
            HostReply::FileSaved(save) => Some(save),
            reply => self.mismatched(request, reply),
        }
    }

    pub fn pick_block(&self, filter: BlockFilter) -> u64 {
        self.ask(HostRequest::PickBlock(filter))
    }

    pub fn list_access(&self, block_id: Uuid) -> u64 {
        self.ask(HostRequest::ListAccess(block_id.into_bytes()))
    }

    pub fn take_access_listing(&self, request: u64) -> Option<AccessListing> {
        match self.take_reply(request)? {
            HostReply::AccessListed(listing) => Some(listing),
            reply => self.mismatched(request, reply),
        }
    }

    pub fn set_access(&self, block_id: Uuid, account: Uuid, access: AccessLevel) {
        self.access_changes
            .borrow_mut()
            .push((block_id, account, access));
        self.changed();
    }

    pub(crate) fn take_access_changes(&self) -> Vec<(Uuid, Uuid, AccessLevel)> {
        std::mem::take(&mut self.access_changes.borrow_mut())
    }

    pub fn take_block_pick(&self, request: u64) -> Option<BlockPick> {
        match self.take_reply(request)? {
            HostReply::BlockPicked(pick) => Some(pick),
            reply => self.mismatched(request, reply),
        }
    }

    pub fn paste_image(&self) -> u64 {
        self.ask(HostRequest::PasteImage)
    }

    pub fn take_pasted_image(&self, request: u64) -> Option<ClipboardImage> {
        match self.take_reply(request)? {
            HostReply::ImagePasted(image) => Some(image),
            reply => self.mismatched(request, reply),
        }
    }

    pub fn play_audio(&self, block_id: Uuid) {
        self.audio_commands
            .borrow_mut()
            .push((block_id, AudioCommand::Toggle));
    }

    pub fn reset_audio(&self, block_id: Uuid) {
        self.audio_commands
            .borrow_mut()
            .push((block_id, AudioCommand::Reset));
    }

    pub fn audio(&self) -> AudioStatus {
        self.audio_status.borrow().clone()
    }

    pub(crate) fn take_audio_commands(&self) -> Vec<(Uuid, AudioCommand)> {
        std::mem::take(&mut self.audio_commands.borrow_mut())
    }

    pub fn set_audio(&self, status: AudioStatus) {
        *self.audio_status.borrow_mut() = status;
        self.push(Pushed::Audio);
    }

    pub fn fetch(&self, url: impl Into<String>) -> u64 {
        self.ask(HostRequest::Fetch(url.into()))
    }

    pub fn take_fetch(&self, request: u64) -> Option<FetchResult> {
        match self.take_reply(request)? {
            HostReply::Fetched(result) => Some(result),
            reply => self.mismatched(request, reply),
        }
    }

    pub fn list_data(&self) -> u64 {
        self.ask(HostRequest::ListData)
    }

    pub fn take_data_listing(&self, request: u64) -> Option<DataListing> {
        match self.take_reply(request)? {
            HostReply::DataListed(listing) => Some(listing),
            reply => self.mismatched(request, reply),
        }
    }

    pub fn read_data(&self, path: impl Into<String>) -> u64 {
        self.ask(HostRequest::ReadData(path.into()))
    }

    pub fn take_data(&self, request: u64) -> Option<FetchResult> {
        match self.take_reply(request)? {
            HostReply::DataRead(result) => Some(result),
            reply => self.mismatched(request, reply),
        }
    }

    pub fn take_requests(&self) -> Vec<(u64, HostRequest)> {
        std::mem::take(&mut self.requests.borrow_mut())
    }

    pub fn set_reply(&self, request: u64, reply: HostReply) {
        self.replies.borrow_mut().insert(request, reply);
        self.push(Pushed::Replies);
    }

    fn take_reply(&self, request: u64) -> Option<HostReply> {
        self.replies.borrow_mut().remove(&request)
    }

    fn mismatched<T>(&self, request: u64, reply: HostReply) -> Option<T> {
        eprintln!("the host answered request {request} with an unrelated {reply:?}");
        None
    }

    pub fn forget_request(&self, request: u64) {
        self.requests.borrow_mut().retain(|(id, _)| *id != request);
        self.replies.borrow_mut().remove(&request);
    }

    pub fn open_web_view(&self, web_view: WebViewId, url: impl Into<String>) {
        self.command_web_view(web_view, WebViewCommand::Open(url.into()));
    }

    pub fn load_web_view(&self, web_view: WebViewId, url: impl Into<String>) {
        self.command_web_view(web_view, WebViewCommand::Load(url.into()));
    }

    pub fn reload_web_view(&self, web_view: WebViewId) {
        self.command_web_view(web_view, WebViewCommand::Reload);
    }

    pub fn close_web_view(&self, web_view: WebViewId) {
        self.command_web_view(web_view, WebViewCommand::Close);
    }

    pub fn focus_app(&self, web_view: WebViewId) {
        self.command_web_view(web_view, WebViewCommand::FocusApp);
    }

    fn command_web_view(&self, web_view: WebViewId, command: WebViewCommand) {
        self.web_view_commands
            .borrow_mut()
            .push((web_view, command));
    }

    pub fn take_web_view_events(&self, web_view: WebViewId) -> Vec<WebViewEvent> {
        let mut events = self.web_view_events.borrow_mut();
        let (taken, kept) = std::mem::take(&mut *events)
            .into_iter()
            .partition(|(id, _)| *id == web_view);
        *events = kept;
        taken.into_iter().map(|(_, event)| event).collect()
    }

    pub fn take_web_view_commands(&self) -> Vec<(WebViewId, WebViewCommand)> {
        std::mem::take(&mut self.web_view_commands.borrow_mut())
    }

    pub fn push_web_view_event(&self, web_view: WebViewId, event: WebViewEvent) {
        self.web_view_events.borrow_mut().push((web_view, event));
        self.push(Pushed::WebView);
    }

    pub fn request_frame_in(&self, delay: Duration) {
        let held = self.next_frame.get();
        if held.is_none_or(|held| delay < held) {
            self.next_frame.set(Some(delay));
        }
    }

    pub fn take_frame_request(&self) -> Option<Duration> {
        self.next_frame.take()
    }

    pub fn grab_cursor(&self, grabbed: bool) {
        if self.cursor_grabbed.replace(grabbed) != grabbed {
            self.cursor_grab_changed.set(true);
        }
    }

    pub fn cursor_grabbed(&self) -> bool {
        self.cursor_grabbed.get()
    }

    pub fn take_cursor_grab(&self) -> Option<bool> {
        self.cursor_grab_changed
            .replace(false)
            .then(|| self.cursor_grabbed.get())
    }

    pub fn chrome_shown(&self) -> bool {
        self.chrome.get().unwrap_or(true)
    }

    pub fn set_chrome_shown(&self, chrome: bool) {
        if self.chrome.replace(Some(chrome)) != Some(chrome) {
            self.changed();
        }
    }

    pub fn copy_text(&self, text: impl Into<String>) {
        self.copied.borrow_mut().push(text.into());
    }

    pub fn take_copied_text(&self) -> Vec<String> {
        std::mem::take(&mut self.copied.borrow_mut())
    }

    pub fn request_paste(&self) {
        self.paste_requested.set(true);
    }

    pub fn take_paste_request(&self) -> bool {
        self.paste_requested.take()
    }

    pub fn leave_frame(&self) {
        self.leaving.set(true);
    }

    pub fn take_leave_frame(&self) -> bool {
        self.leaving.take()
    }

    pub fn bar_action(&self, action: BarAction) {
        self.bar_actions.borrow_mut().push(action);
        self.changed();
    }

    pub fn take_bar_actions(&self) -> Vec<BarAction> {
        std::mem::take(&mut self.bar_actions.borrow_mut())
    }

    pub fn set_menu(&self, entries: Vec<MenuEntry>) {
        if *self.menu.borrow() == entries {
            return;
        }
        *self.menu.borrow_mut() = entries;
        self.changed();
    }

    pub(crate) fn menu(&self) -> Vec<MenuEntry> {
        self.menu.borrow().clone()
    }

    pub(crate) fn push_menu_pick(&self, id: String) {
        self.menu_picks.borrow_mut().push(id);
        self.changed();
    }

    pub fn take_menu_picks(&self) -> Vec<String> {
        std::mem::take(&mut self.menu_picks.borrow_mut())
    }

    pub fn pick_child_menu(&self, child: ChildId, id: String) {
        self.child_menu_picks.borrow_mut().push((child, id));
        self.changed();
    }

    pub(crate) fn take_child_menu_picks(&self) -> Vec<(ChildId, String)> {
        std::mem::take(&mut self.child_menu_picks.borrow_mut())
    }

    pub fn presenting(&self) -> bool {
        self.presenting.get()
    }

    pub fn present(&self, presenting: bool) {
        if self.presenting.get() == presenting {
            return;
        }
        self.present_requests.borrow_mut().push(presenting);
    }

    pub fn occlude(&self, rect: Rect) {
        let origin = self.region.get().origin;
        let mut children = self.children.borrow_mut();
        let after = children.placements.len() as u32;
        children.occluders.push(Occluder {
            after,
            rect: child_rect(rect.translate(-origin)),
        });
    }

    pub fn place_child(
        &self,
        content: ChildContent,
        rect: Rect,
        clip: Rect,
        mode: ChildMode,
        layer: ChildLayer,
        own_frame: bool,
        top_bar: TopBar,
        rotation: f32,
        opacity: f32,
        intrinsic: Option<Vec2>,
    ) -> ChildId {
        let state = self.region.get();
        let clip = clip.intersect(swept(rect, rotation));
        let mut children = self.children.borrow_mut();
        let child = children.identify(
            state.region.unwrap_or(EditorRegion::Frame),
            Identity::of(&content),
        );
        children.placements.push(ChildPlacement {
            child,
            content,
            rect: child_rect(rect.translate(-state.origin)),
            clip: child_rect(clip.translate(-state.origin)),
            own_frame,
            top_bar,
            corner_radius: 0.0,
            layer,
            mode,
            intrinsic: intrinsic.map(|size| Size {
                width: size.x.max(0.0),
                height: size.y.max(0.0),
            }),
            rotation,
            opacity: opacity.clamp(0.0, 1.0),
        });
        child
    }

    pub fn child_status(&self, child: ChildId) -> Option<ChildStatus> {
        self.child_statuses.borrow().get(&child).cloned()
    }

    pub fn set_creation_ready(&self, ready: bool) {
        if self.creation_ready.get() == ready {
            return;
        }
        self.creation_ready.set(ready);
        self.creation_changed.set(true);
    }

    pub fn take_opens(&self) -> Vec<OpenRequest> {
        std::mem::take(&mut self.opens.borrow_mut())
    }

    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        *self.block_types.borrow_mut() = catalog;
        self.push(Pushed::Catalog);
    }

    pub fn set_client_id(&self, client_id: Uuid) {
        self.client_id.set(client_id);
    }

    pub fn set_account_id(&self, account: Uuid) {
        self.account.set(account);
    }

    pub fn account_id(&self) -> Uuid {
        self.account.get()
    }

    pub fn set_workspace_id(&self, workspace: Uuid) {
        self.workspace.set(workspace);
    }

    pub fn workspace_id(&self) -> Uuid {
        self.workspace.get()
    }

    pub fn blocks(&self) -> crate::graph::Blocks {
        crate::graph::Blocks {
            graph: Rc::clone(&self.graph),
            waker: self.waker.clone(),
            account: Rc::clone(&self.account),
        }
    }

    pub fn flush_graph(&self) {
        self.graph.flush();
    }

    pub fn defer_graph_changes_unless(&self, ready: impl Fn() -> bool + 'static) {
        self.graph.defer_unless(Rc::new(ready));
    }

    pub fn set_blocks(&self, query: crate::BlockQuery, blocks: Vec<crate::BlockInfo>) {
        self.graph.set_result(query, blocks);
        self.waker.wake();
    }

    pub fn watched_blocks(&self) -> Vec<crate::BlockQuery> {
        self.graph.watched()
    }

    pub fn take_block_watch(&self) -> Option<Vec<crate::BlockQuery>> {
        self.graph.take_watch()
    }

    pub fn take_graph_commands(&self) -> Vec<crate::GraphCommand> {
        self.graph.take_commands()
    }

    pub fn set_editable(&self, editable: bool) {
        self.editable.set(editable);
        self.changed();
    }

    pub fn set_view(&self, view: Rect, scale: f32) {
        self.view.set(Some(View { rect: view, scale }));
        self.changed();
    }

    pub fn report_content(&self, rect: Rect) {
        self.content.set(Some(rect));
    }

    pub fn take_content(&self) -> Option<Rect> {
        self.content.take()
    }

    pub fn take_view_changes(&self) -> Vec<ViewChange> {
        std::mem::take(&mut self.view_changes.borrow_mut())
    }

    pub fn set_drag(&self, drag: Option<BlockDrag>) {
        self.drag.set(drag);
        self.changed();
    }

    pub fn take_drag_accepted(&self) -> Option<bool> {
        self.drag_accepted.take()
    }

    pub fn begin_region(&self, region: EditorRegion, origin: Vec2) {
        self.region.set(Region {
            region: Some(region),
            origin,
        });
        let mut children = self.children.borrow_mut();
        children.placements.clear();
        children.occluders.clear();
        children.ordinals.clear();
        children.used.clear();
    }

    pub fn end_region(&self, region: EditorRegion) -> (Vec<ChildPlacement>, Vec<Occluder>) {
        self.region.set(Region::default());
        let mut children = self.children.borrow_mut();
        let used = std::mem::take(&mut children.used);
        children
            .identities
            .retain(|key, _| key.0 != region || used.contains(key));
        (
            std::mem::take(&mut children.placements),
            std::mem::take(&mut children.occluders),
        )
    }

    pub fn set_child_statuses(&self, statuses: Vec<ChildStatus>) {
        let mut current = self.child_statuses.borrow_mut();
        for status in statuses {
            current.insert(status.child, status);
        }
        drop(current);
        self.changed();
    }

    pub fn retain_child_statuses(&self, live: &HashSet<ChildId>) {
        self.child_statuses
            .borrow_mut()
            .retain(|child, _| live.contains(child));
    }

    pub fn set_presenting(&self, presenting: bool) {
        self.presenting.set(presenting);
        self.changed();
    }

    pub fn take_child_view_changes(&self, child: ChildId) -> Vec<ViewChange> {
        self.child_views
            .borrow_mut()
            .remove(&child)
            .unwrap_or_default()
    }

    pub fn take_child_bar_actions(&self, child: ChildId) -> Vec<BarAction> {
        self.child_bars
            .borrow_mut()
            .remove(&child)
            .unwrap_or_default()
    }

    pub(crate) fn push_child_bar_action(&self, child: ChildId, action: BarAction) {
        self.child_bars
            .borrow_mut()
            .entry(child)
            .or_default()
            .push(action);
        self.changed();
    }

    pub(crate) fn push_child_view_change(&self, child: ChildId, change: ViewChange) {
        self.child_views
            .borrow_mut()
            .entry(child)
            .or_default()
            .push(change);
        self.changed();
    }

    pub(crate) fn take_present_requests(&self) -> Vec<bool> {
        std::mem::take(&mut self.present_requests.borrow_mut())
    }

    pub(crate) fn take_creation_ready(&self) -> Option<bool> {
        self.creation_changed
            .take()
            .then(|| self.creation_ready.get())
    }

    pub(crate) fn take_performance(&self) -> Vec<(String, Vec<PerformanceMeasurement>)> {
        let records = std::mem::take(
            &mut *self
                .performance
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        let mut groups = std::collections::BTreeMap::<Arc<str>, Vec<PerformanceMeasurement>>::new();
        for record in records {
            groups
                .entry(record.group)
                .or_default()
                .push(record.measurement);
        }
        groups
            .into_iter()
            .map(|(group, measurements)| (group.to_string(), measurements))
            .collect()
    }
}

#[derive(Default)]
pub struct FileSaver {
    request: Option<u64>,
}

impl FileSaver {
    pub fn save(&mut self, host: &EditorHost, file: SavedFile) {
        self.request = Some(host.save_file(file));
    }

    pub fn is_saving(&self) -> bool {
        self.request.is_some()
    }

    pub fn poll(&mut self, host: &EditorHost) -> Option<Result<bool, String>> {
        let save = host.take_save(self.request?)?;
        self.request = None;
        Some(match save {
            FileSave::Saved => Ok(true),
            FileSave::Cancelled => Ok(false),
            FileSave::Failed(error) => Err(error),
        })
    }
}

#[derive(Default)]
pub struct ImagePaster {
    request: Option<u64>,
}

impl ImagePaster {
    pub fn paste(&mut self, host: &EditorHost, asked: bool) -> Option<PastedImage> {
        if let Some(request) = self.request {
            return match host.take_pasted_image(request)? {
                ClipboardImage::Pasted { name, data } => {
                    self.request = None;
                    Some(PastedImage::Image { name, data })
                }
                ClipboardImage::Empty => {
                    self.request = None;
                    Some(PastedImage::Empty)
                }
                ClipboardImage::Failed(error) => {
                    self.request = None;
                    Some(PastedImage::Failed(error))
                }
            };
        }
        if !asked {
            return None;
        }
        self.request = Some(host.paste_image());
        None
    }
}

pub enum PastedImage {
    Image { name: String, data: Vec<u8> },
    Empty,
    Failed(String),
}

fn child_rect(rect: Rect) -> ChildRect {
    ChildRect {
        x: rect.min.x,
        y: rect.min.y,
        width: rect.width().max(0.0),
        height: rect.height().max(0.0),
    }
}

#[derive(Default)]
pub struct BlockPicker {
    request: Option<u64>,
}

impl BlockPicker {
    pub fn open(&mut self, host: &EditorHost, filter: BlockFilter) {
        self.request = Some(host.pick_block(filter));
    }

    pub fn is_open(&self) -> bool {
        self.request.is_some()
    }

    pub fn poll(&mut self, host: &EditorHost) -> Option<Result<PickedBlock, String>> {
        let pick = host.take_block_pick(self.request?)?;
        self.request = None;
        match pick {
            BlockPick::Chosen {
                block_id,
                block_type,
                linked,
                placed,
            } => Some(Ok(PickedBlock {
                id: Uuid::from_bytes(block_id),
                block_type: Uuid::from_bytes(block_type),
                linked,
                placed,
            })),
            BlockPick::Cancelled => None,
            BlockPick::Failed(error) => Some(Err(error)),
        }
    }
}

#[cfg(test)]
mod tests;
