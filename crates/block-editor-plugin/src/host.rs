use stable_hash::{HashMap, HashSet};
use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::asked::{Asked, Task, asking};
use crate::graph::BlockParent;
use crate::state::Subscription;
use block_plugin_api::TopBar;
use block_plugin_api::guest::host_services;
use block_plugin_api::{
    AccessLevel, AccessListing, ArtifactAction, AudioCommand, AudioSource, AudioStatus, BarAction,
    BlockCommand, BlockPick, ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement,
    ChildStatus, DialogAnswer, DialogFrame, EditorInstanceId, Failure, FilePick, FileSave, FitArea,
    HostPanel, HostView, HostWindowId, MenuEntry, Occluder, ParentView, PerformanceMeasurement,
    Revision, ShellCommand, ShellDialog, Size, ViewChange, ViewId, ViewPlacement, Watched,
    WebViewCommand, WebViewEvent, WebViewId,
};
pub use block_plugin_api::{BlockFilter, FileFilter, SavedFile};
use block_type_guest::editor::{EditorOperation, FromEditor, Snapshot, ToEditor};
use block_ui::BlockCatalog;
use geometry::{Pos2, Rect, Vec2};
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
    pub unsettled: Vec<Vec<u8>>,
    pub applied: u64,
}

impl HostContent {
    pub fn of<C: be_block::LiveEdit>(content: &C) -> Self {
        Self {
            content_type: C::CONTENT_TYPE,
            bytes: content.encode(),
            session: content.session_state(),
            unsettled: Vec::new(),
            applied: 0,
        }
    }

    pub fn snapshot(self) -> Snapshot {
        Snapshot {
            content_type: self.content_type,
            bytes: self.bytes,
            session: self.session,
            unsettled: self.unsettled,
            applied: self.applied,
        }
    }
}

type ContentOperation = (Option<Uuid>, FromEditor);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShownPresence {
    pub block: Option<Uuid>,
    pub kind: Uuid,
    pub value: Option<Vec<u8>>,
}

type Peers = (u64, Vec<PeerPresence>);

const PEER_MESSAGE_LIMIT: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerMessage {
    pub from: u64,
    pub channel: Uuid,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SentToPeers {
    pub block: Option<Uuid>,
    pub to: Option<u64>,
    pub channel: Uuid,
    pub payload: Vec<u8>,
}

#[derive(Default)]
struct PeerMessages {
    received: HashMap<Option<Uuid>, std::collections::VecDeque<(u64, PeerMessage)>>,
    count: u64,
    sent: Vec<SentToPeers>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerPresence {
    pub client: u64,
    pub kind: Uuid,
    pub value: Vec<u8>,
}

#[derive(Clone, PartialEq)]
pub struct FileDrop {
    pub position: Pos2,
    pub files: Vec<PickedFile>,
    pub dropped: bool,
}

#[derive(Clone)]
pub(crate) enum Dragged {
    Block(BlockDrag),
    Files(FileDrop),
}

impl Dragged {
    pub(crate) fn dropped(&self) -> bool {
        match self {
            Self::Block(drag) => drag.dropped,
            Self::Files(drop) => drop.dropped,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenRequest {
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub via: Option<Uuid>,
    pub editor: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChildCommit {
    pub(crate) child: ChildId,
    pub(crate) parent: BlockParent,
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickerRequest {
    pub filter: BlockFilter,
    pub parent: BlockParent,
    pub sheet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenameRequest {
    pub block: Uuid,
    pub sheet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShowRequest {
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub via: Option<Uuid>,
    pub editor: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockHistory {
    pub head: Option<[u8; 32]>,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredHead {
    pub commit: [u8; 32],
    pub content_type: Uuid,
    pub bytes: Vec<u8>,
}

pub type HeadRead = Result<Option<StoredHead>, Failure>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArtifactState {
    pub block_id: Uuid,
    pub source_type: Uuid,
    pub source: Uuid,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub regenerating: bool,
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
    pub source: Uuid,
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

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum Identity {
    Block([u8; 16]),
    WebView(WebViewId),
    Host(HostPanel),
    Window(HostWindowId),
    Creation([u8; 16], String),
    ArtifactSettings([u8; 16]),
    DockTab(block_plugin_api::DockTabPath),
    Picker(BlockFilter),
    Rename([u8; 16]),
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
            ChildContent::DockTab(tab) => Self::DockTab(tab.clone()),
            ChildContent::Picker { filter, .. } => Self::Picker(filter.clone()),
            ChildContent::Rename(block_id) => Self::Rename(*block_id),
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum ChildScope {
    View(ViewId),
    DockTab(u64),
}

type ChildKey = (ChildScope, Identity, u32);

#[derive(Default)]
struct Placing {
    placements: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    ordinals: HashMap<(ChildScope, Identity), u32>,
    used: Vec<ChildKey>,
}

#[derive(Default)]
struct Children {
    placing: HashMap<ViewId, Placing>,
    identities: HashMap<ChildKey, ChildId>,
    tab_views: HashMap<u64, ViewId>,
    next: u64,
}

impl Children {
    fn identify(&mut self, view: ViewId, dock_tab: Option<u64>, identity: Identity) -> ChildId {
        let scope = match dock_tab {
            Some(tab) => {
                self.tab_views.insert(tab, view);
                ChildScope::DockTab(tab)
            }
            None => ChildScope::View(view),
        };
        let placing = self.placing.entry(view).or_default();
        let ordinal = {
            let ordinal = placing
                .ordinals
                .entry((scope, identity.clone()))
                .or_default();
            let current = *ordinal;
            *ordinal += 1;
            current
        };
        let key = (scope, identity, ordinal);
        let child = match self.identities.get(&key) {
            Some(child) => *child,
            None => {
                self.next += 1;
                let child = ChildId(self.next);
                self.identities.insert(key.clone(), child);
                child
            }
        };
        self.placing.entry(view).or_default().used.push(key);
        child
    }

    fn placed_in(&self, scope: ChildScope, view: ViewId) -> bool {
        match scope {
            ChildScope::View(held) => held == view,
            ChildScope::DockTab(tab) => self.tab_views.get(&tab) == Some(&view),
        }
    }
}

#[derive(Default)]
struct ViewDrops {
    dragged: Option<Dragged>,
    content: Option<Rect>,
}

#[derive(Clone, Copy)]
struct View {
    rect: Rect,
    scale: f32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Pushed {
    Peers,
    Histories,
    Artifacts,
    Audio,
    Focus,
    Catalog,
    WebView,
    Shows,
    Version,
    Watches,
    PeerMessages,
    Heads,
    Docks,
}

impl Pushed {
    pub const ALL: [Self; 13] = [
        Self::Peers,
        Self::Histories,
        Self::Artifacts,
        Self::Audio,
        Self::Focus,
        Self::Catalog,
        Self::WebView,
        Self::Shows,
        Self::Version,
        Self::Watches,
        Self::PeerMessages,
        Self::Heads,
        Self::Docks,
    ];
}

type Opening = Box<dyn FnOnce(&EditorHost, EditorInstanceId)>;

#[derive(Default)]
struct Watches {
    values: HashMap<TypeId, Rc<dyn Any>>,
    revisions: HashMap<TypeId, u64>,
    watched: HashSet<TypeId>,
    changed: crate::state::Callbacks<TypeId, dyn Fn()>,
    opening: Vec<Opening>,
    calls: Vec<Box<dyn FnOnce()>>,
}

#[derive(Clone, Default)]
pub struct EditorHost {
    waker: Waker,
    shown_panels: Rc<RefCell<Vec<HostPanel>>>,
    watches: Rc<RefCell<Watches>>,
    picker: Rc<RefCell<Option<PickerRequest>>>,
    renaming: Rc<Cell<Option<RenameRequest>>>,
    picked: Rc<RefCell<Option<BlockPick>>>,
    card: Rc<Cell<bool>>,
    dialog_requests: Rc<RefCell<Vec<(Uuid, ShellDialog)>>>,
    child_commits: Rc<RefCell<Vec<ChildCommit>>>,
    pushed: Rc<[Cell<u64>; Pushed::ALL.len()]>,
    on_pushed: Rc<crate::state::Callbacks<Pushed, dyn Fn()>>,
    extensions: Rc<RefCell<HashMap<TypeId, Rc<dyn Any>>>>,
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
    heads: Rc<RefCell<HashMap<Uuid, HeadRead>>>,
    watched_heads: Rc<RefCell<Vec<Uuid>>>,
    reported_heads: Rc<RefCell<Option<Vec<Uuid>>>>,
    block_drags: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
    block_commands: Rc<RefCell<Vec<(Uuid, BlockCommand)>>>,
    shell_commands: Rc<RefCell<Vec<(Uuid, ShellCommand)>>>,
    simulated_access: Rc<RefCell<Vec<(Uuid, AccessLevel)>>>,
    version_commands: Rc<RefCell<Vec<(Uuid, block_plugin_api::VersionCommand)>>>,
    version_status: Rc<RefCell<Option<block_plugin_api::VersionStatus>>>,
    version_watched: Rc<Cell<bool>>,
    shows_watched: Rc<Cell<bool>>,
    block_types: Rc<RefCell<Rc<BlockCatalog>>>,
    block_types_known: Rc<Cell<bool>>,
    drag_accepted: Rc<Cell<Option<bool>>>,
    instance: Rc<Cell<Option<EditorInstanceId>>>,
    tasks: Rc<RefCell<Vec<Task>>>,
    editable: Rc<Cell<bool>>,
    block_type: Rc<Cell<Option<Uuid>>>,
    client_id: Rc<Cell<Uuid>>,
    view_block: Rc<Cell<Option<Uuid>>>,
    view: Rc<Cell<Option<View>>>,
    view_changes: Rc<RefCell<Vec<ViewChange>>>,
    handles_dismiss: Rc<Cell<bool>>,
    handles_dismiss_changed: Rc<Cell<bool>>,
    dialog_closes: Rc<RefCell<Vec<DialogAnswer>>>,
    dismiss_requested: Rc<Cell<bool>>,
    child_dismissals: Rc<RefCell<Vec<ChildId>>>,
    performance: Arc<Mutex<Vec<PerformanceRecord>>>,
    showing: Rc<Cell<Option<block_plugin_api::View>>>,
    drops: Rc<RefCell<HashMap<ViewId, ViewDrops>>>,
    children: Rc<RefCell<Children>>,
    child_statuses: Rc<RefCell<HashMap<ChildId, ChildStatus>>>,
    audio_commands: Rc<RefCell<Vec<AudioCommand>>>,
    audio_status: Rc<RefCell<AudioStatus>>,
    web_view_commands: Rc<RefCell<Vec<(WebViewId, WebViewCommand)>>>,
    web_view_events: Rc<RefCell<Vec<(WebViewId, WebViewEvent)>>>,
    presenting: Rc<Cell<bool>>,
    present_requests: Rc<RefCell<Vec<bool>>>,
    child_bars: Rc<RefCell<HashMap<ChildId, Vec<BarAction>>>>,
    child_focuses: Rc<RefCell<HashSet<ChildId>>>,
    bar_actions: Rc<RefCell<Vec<BarAction>>>,
    menus: Rc<RefCell<std::collections::BTreeMap<ViewId, Vec<MenuEntry>>>>,
    menu_picks: Rc<RefCell<Vec<(ViewId, String)>>>,
    child_menu_picks: Rc<RefCell<Vec<(ChildId, String)>>>,
    chrome: Rc<Cell<Option<bool>>>,
    placed_content: Rc<Cell<Option<Rect>>>,
    frame_anchor: Rc<Cell<Option<Rect>>>,
    spec_anchor: Rc<Cell<Option<Option<Rect>>>>,
    copied: Rc<RefCell<Vec<String>>>,
    copied_images: Rc<RefCell<Vec<Vec<u8>>>>,
    paste_requested: Rc<Cell<bool>>,
    leaving: Rc<Cell<bool>>,
    next_frame: Rc<RefCell<HashMap<Option<ViewId>, Duration>>>,
    content_updates: Rc<RefCell<HashMap<Option<Uuid>, Vec<ToEditor>>>>,
    followed: Rc<RefCell<HashMap<Option<Uuid>, usize>>>,
    asked: Rc<RefCell<HashSet<Option<Uuid>>>>,
    content_operations: Rc<RefCell<Vec<ContentOperation>>>,
    resend_requests: Rc<RefCell<Vec<Option<Uuid>>>>,
    watched_content: Rc<RefCell<std::collections::BTreeMap<Uuid, Uuid>>>,
    seeded: Rc<RefCell<Vec<SeededContent>>>,
    shown: Rc<RefCell<Vec<ShownPresence>>>,
    peers: Rc<RefCell<HashMap<Option<Uuid>, Peers>>>,
    next_peers: Rc<Cell<u64>>,
    peer_messages: Rc<RefCell<PeerMessages>>,
    reported_content: Rc<RefCell<Option<std::collections::BTreeMap<Uuid, Uuid>>>>,
    graph: Rc<crate::graph::GraphState>,
    account: Rc<Cell<Uuid>>,
    organization: Rc<Cell<Uuid>>,
    module: Rc<RefCell<std::rc::Weak<crate::module::ModuleState>>>,
    settings_draft: Rc<RefCell<Option<crate::SettingsDraft>>>,
    pub(crate) docks: Rc<RefCell<crate::docks::Docks>>,
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

    pub(crate) fn push(&self, pushed: Pushed) {
        let revision = &self.pushed[pushed as usize];
        revision.set(revision.get() + 1);
        self.changed();
        for run in self.on_pushed.of([&pushed]) {
            run();
        }
    }

    pub fn on_pushed(&self, pushed: Pushed, run: impl Fn() + 'static) -> Subscription {
        self.on_pushed.add(pushed, Rc::new(run))
    }

    pub fn extension<T: Default + 'static>(&self) -> Rc<T> {
        let held = self
            .extensions
            .borrow()
            .get(&TypeId::of::<T>())
            .cloned()
            .and_then(|held| held.downcast::<T>().ok());
        if let Some(held) = held {
            return held;
        }
        let made = Rc::new(T::default());
        self.extensions
            .borrow_mut()
            .insert(TypeId::of::<T>(), Rc::clone(&made) as Rc<dyn Any>);
        made
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
        self.push_open(block_id, block_type, None, None);
    }

    pub fn open_block_with(&self, block_id: Uuid, block_type: Uuid, editor: Uuid) {
        self.push_open(block_id, block_type, None, Some(editor));
    }

    pub fn open_block_via(&self, block_id: Uuid, block_type: Uuid, container: Uuid) {
        self.push_open(block_id, block_type, Some(container), None);
    }

    fn push_open(&self, block_id: Uuid, block_type: Uuid, via: Option<Uuid>, editor: Option<Uuid>) {
        self.opens.borrow_mut().push(OpenRequest {
            block_id,
            block_type,
            via,
            editor,
        });
    }

    pub fn take_show_requests(&self) -> Vec<ShowRequest> {
        self.shows_watched.set(true);
        std::mem::take(&mut self.shows.borrow_mut())
    }

    pub(crate) fn wants_shows(&self) -> bool {
        self.shows_watched.get()
    }

    pub fn take_dialog_requests(&self) -> Vec<(Uuid, ShellDialog)> {
        std::mem::take(&mut self.dialog_requests.borrow_mut())
    }

    pub fn show_dialog(&self, block_id: Uuid, dialog: ShellDialog) {
        self.dialog_requests.borrow_mut().push((block_id, dialog));
        self.push(Pushed::Shows);
    }

    pub fn renaming(&self) -> Option<RenameRequest> {
        self.renaming.get()
    }

    pub(crate) fn set_renaming(&self, rename: RenameRequest) {
        self.renaming.set(Some(rename));
        self.push(Pushed::Shows);
    }

    pub fn picker(&self) -> Option<PickerRequest> {
        self.picker.borrow().clone()
    }

    pub(crate) fn set_picker(&self, picker: PickerRequest) {
        *self.picker.borrow_mut() = Some(picker);
        self.push(Pushed::Shows);
    }

    pub fn answer_pick(&self, answer: BlockPick) {
        let mut picked = self.picked.borrow_mut();
        if picked.is_none() {
            *picked = Some(answer);
        }
        drop(picked);
        self.changed();
    }

    pub fn show_card(&self, card: bool) {
        if self.card.replace(card) != card {
            self.changed();
        }
    }

    pub(crate) fn card(&self) -> bool {
        self.card.get()
    }

    pub(crate) fn take_picked(&self) -> Option<BlockPick> {
        self.picked.borrow_mut().take()
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
        self.shows_watched.set(true);
        std::mem::take(&mut self.shown_panels.borrow_mut())
    }

    pub fn show_panel(&self, panel: HostPanel) {
        self.shown_panels.borrow_mut().push(panel);
        self.push(Pushed::Shows);
    }

    pub fn watched<W: Watched>(&self) -> Option<W::Value> {
        self.watch::<W>();
        self.watches
            .borrow()
            .values
            .get(&TypeId::of::<W>())
            .and_then(|value| value.downcast_ref::<W::Value>())
            .cloned()
    }

    pub fn offers<W: Watched>(&self) -> bool {
        self.watch::<W>();
        self.watches
            .borrow()
            .values
            .contains_key(&TypeId::of::<W>())
    }

    fn watch<W: Watched>(&self) {
        let mut watches = self.watches.borrow_mut();
        if !watches.watched.insert(TypeId::of::<W>()) {
            return;
        }
        watches
            .opening
            .push(Box::new(|host: &EditorHost, instance| {
                let host = host.clone();
                crate::asked::follow::<W>(Some(instance), move |value| {
                    host.set_watched::<W>(&value)
                });
            }));
    }

    pub fn set_watched<W: Watched>(&self, value: &W::Value) {
        let mut watches = self.watches.borrow_mut();
        let key = TypeId::of::<W>();
        if watches
            .values
            .get(&key)
            .and_then(|held| held.downcast_ref::<W::Value>())
            == Some(value)
        {
            return;
        }
        *watches.revisions.entry(key).or_default() += 1;
        watches.values.insert(key, Rc::new(value.clone()));
        let runs = watches.changed.of([&key]);
        drop(watches);
        self.push(Pushed::Watches);
        for run in runs {
            run();
        }
    }

    pub fn on_watched<W: Watched>(&self, run: impl Fn() + 'static) -> Subscription {
        self.on_watched_of(TypeId::of::<W>(), run)
    }

    pub fn on_watched_of(&self, watch: TypeId, run: impl Fn() + 'static) -> Subscription {
        self.watches.borrow().changed.add(watch, Rc::new(run))
    }

    pub fn watch_revision<W: Watched>(&self) -> u64 {
        self.watch_revision_of(TypeId::of::<W>())
    }

    pub fn watch_revision_of(&self, watch: TypeId) -> u64 {
        self.watches
            .borrow()
            .revisions
            .get(&watch)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn take_watch_openings(&self) -> Vec<Opening> {
        std::mem::take(&mut self.watches.borrow_mut().opening)
    }

    pub fn act(&self, call: impl FnOnce() + 'static) {
        self.watches.borrow_mut().calls.push(Box::new(call));
        self.changed();
    }

    pub(crate) fn take_acts(&self) -> Vec<Box<dyn FnOnce()>> {
        std::mem::take(&mut self.watches.borrow_mut().calls)
    }

    pub fn show_block(&self, request: ShowRequest) {
        self.shows.borrow_mut().push(request);
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
        let mut watched = self.watched_history.borrow_mut();
        watched.extend(blocks);
        watched.sort();
        watched.dedup();
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

    pub fn set_history(&self, block_id: Uuid, history: BlockHistory) {
        self.histories.borrow_mut().insert(block_id, history);
        self.push(Pushed::Histories);
    }

    pub fn watch_head(&self, block: Uuid) {
        let mut watched = self.watched_heads.borrow_mut();
        if !watched.contains(&block) {
            watched.push(block);
            watched.sort();
        }
    }

    pub fn head(&self, block: Uuid) -> Option<HeadRead> {
        self.heads.borrow().get(&block).cloned()
    }

    pub(crate) fn take_head_watch(&self) -> Option<Vec<Uuid>> {
        let blocks = self.watched_heads.borrow().clone();
        let mut reported = self.reported_heads.borrow_mut();
        if reported
            .as_ref()
            .map_or(blocks.is_empty(), |reported| *reported == blocks)
        {
            return None;
        }
        *reported = Some(blocks.clone());
        Some(blocks)
    }

    pub fn set_head(&self, block: Uuid, head: HeadRead) {
        self.heads.borrow_mut().insert(block, head);
        self.push(Pushed::Heads);
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

    pub fn set_artifact(&self, block_id: Uuid, state: Option<ArtifactState>) {
        let mut artifacts = self.artifacts.borrow_mut();
        match state {
            Some(state) => artifacts.insert(block_id, state),
            None => artifacts.remove(&block_id),
        };
        drop(artifacts);
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
        self.shell_command(block_id, ShellCommand::CloseEditor);
    }

    pub fn simulate_access(&self, block_id: Uuid, access: AccessLevel) {
        self.simulated_access.borrow_mut().push((block_id, access));
    }

    pub(crate) fn take_simulated_access(&self) -> Vec<(Uuid, AccessLevel)> {
        std::mem::take(&mut self.simulated_access.borrow_mut())
    }

    fn shell_command(&self, block_id: Uuid, command: ShellCommand) {
        self.shell_commands.borrow_mut().push((block_id, command));
    }

    pub fn take_shell_commands(&self) -> Vec<(Uuid, ShellCommand)> {
        std::mem::take(&mut self.shell_commands.borrow_mut())
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
        self.shell_command(block_id, ShellCommand::Share);
    }

    pub fn leave_block(&self, block_id: Uuid) {
        self.shell_command(block_id, ShellCommand::Leave);
    }

    pub fn unlink_block(&self, block_id: Uuid, container: Uuid) {
        self.block_commands.borrow_mut().push((
            block_id,
            BlockCommand::Unlink {
                container: container.into_bytes(),
            },
        ));
    }

    pub fn restore_block(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::Restore));
    }

    pub fn delete_block(&self, block_id: Uuid, block_type: Uuid, source: BlockParent) {
        self.block_commands.borrow_mut().push((
            block_id,
            BlockCommand::Delete {
                block_type: block_type.into_bytes(),
                source: source.encode(),
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
        *self.version_status.borrow_mut() = Some(status);
        self.push(Pushed::Version);
    }

    pub fn watch_version(&self) {
        self.version_watched.set(true);
    }

    pub(crate) fn wants_version(&self) -> bool {
        self.version_watched.get()
    }

    pub fn version_status(&self) -> block_plugin_api::VersionStatus {
        self.version_status.borrow().clone().unwrap_or_default()
    }

    pub fn version_known(&self) -> bool {
        self.version_status.borrow().is_some()
    }

    pub fn take_block_commands(&self) -> Vec<(Uuid, BlockCommand)> {
        std::mem::take(&mut self.block_commands.borrow_mut())
    }

    pub fn block_types_known(&self) -> bool {
        self.block_types_known.get()
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
        self.update_content(None, ToEditor::Snapshot(content.snapshot()));
    }

    pub fn set_content_of(&self, block: Uuid, content: HostContent) {
        self.update_content(Some(block), ToEditor::Snapshot(content.snapshot()));
    }

    pub fn push_content_operations(&self, operations: Vec<EditorOperation>) {
        self.update_content(None, ToEditor::Operations(operations));
    }

    pub fn push_content_operations_of(&self, block: Uuid, operations: Vec<EditorOperation>) {
        self.update_content(Some(block), ToEditor::Operations(operations));
    }

    pub fn receive_from_replica(&self, block: Option<Uuid>, message: ToEditor) {
        match message {
            ToEditor::Presence(peers) => self.set_peers(
                block,
                peers
                    .into_iter()
                    .map(|peer| PeerPresence {
                        client: peer.client,
                        kind: peer.kind,
                        value: peer.value,
                    })
                    .collect(),
            ),
            ToEditor::PeerMessages(messages) => self.push_peer_messages(
                block,
                messages
                    .into_iter()
                    .map(|message| PeerMessage {
                        from: message.from,
                        channel: message.channel,
                        payload: message.payload,
                    })
                    .collect(),
            ),
            content @ (ToEditor::Snapshot(_) | ToEditor::Operations(_)) => {
                if self.followed.borrow().contains_key(&block) {
                    self.update_content(block, content);
                }
            }
        }
    }

    fn update_content(&self, block: Option<Uuid>, update: ToEditor) {
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

    pub fn take_content_updates(&self, block: Option<Uuid>) -> Vec<ToEditor> {
        self.content_updates
            .borrow_mut()
            .remove(&block)
            .unwrap_or_default()
    }

    pub(crate) fn follow(&self, block: Option<Uuid>) {
        *self.followed.borrow_mut().entry(block).or_default() += 1;
        self.asked.borrow_mut().insert(block);
        self.request_content_resend(block);
    }

    pub(crate) fn unfollow(&self, block: Option<Uuid>) {
        let mut followed = self.followed.borrow_mut();
        let Some(count) = followed.get_mut(&block) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            followed.remove(&block);
            self.content_updates.borrow_mut().remove(&block);
        }
    }

    pub fn ask_for_peers(&self, block: Option<Uuid>) {
        if self.asked.borrow_mut().insert(block) {
            self.request_content_resend(block);
        }
    }

    pub(crate) fn send_to_replica(&self, block: Option<Uuid>, message: FromEditor) {
        match message {
            FromEditor::Resend => self.request_content_resend(block),
            FromEditor::ShowPresence { kind, value } => self.show_presence(block, kind, value),
            FromEditor::SendToPeers {
                to,
                channel,
                payload,
            } => self.send_to_peers(block, to, channel, payload),
            message
            @ (FromEditor::Operate(_) | FromEditor::Gesture(_) | FromEditor::Replace(_)) => {
                self.content_operations.borrow_mut().push((block, message));
            }
        }
    }

    pub fn operate_content(&self, operation: Vec<u8>) {
        self.send_to_replica(None, FromEditor::Operate(operation));
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
        taken
            .into_iter()
            .filter_map(|(_, outgoing)| match outgoing {
                FromEditor::Operate(operation) => Some(operation),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn take_outgoing(
        &self,
        mut send: impl FnMut(Option<Uuid>, FromEditor) -> Option<FromEditor>,
    ) {
        let resends = std::mem::take(&mut *self.resend_requests.borrow_mut());
        let operations = std::mem::take(&mut *self.content_operations.borrow_mut());
        let shown = std::mem::take(&mut *self.shown.borrow_mut());
        let sent = std::mem::take(&mut self.peer_messages.borrow_mut().sent);
        let outgoing = resends
            .into_iter()
            .map(|block| (block, FromEditor::Resend))
            .chain(operations)
            .chain(shown.into_iter().map(|shown| {
                let message = FromEditor::ShowPresence {
                    kind: shown.kind,
                    value: shown.value,
                };
                (shown.block, message)
            }))
            .chain(sent.into_iter().map(|sent| {
                let message = FromEditor::SendToPeers {
                    to: sent.to,
                    channel: sent.channel,
                    payload: sent.payload,
                };
                (sent.block, message)
            }));
        for (block, message) in outgoing {
            if let Some(kept) = send(block, message) {
                self.keep_outgoing(block, kept);
            }
        }
    }

    fn keep_outgoing(&self, block: Option<Uuid>, message: FromEditor) {
        match message {
            FromEditor::Resend => {
                let mut requests = self.resend_requests.borrow_mut();
                if !requests.contains(&block) {
                    requests.push(block);
                }
            }
            FromEditor::ShowPresence { kind, value } => {
                let mut shown = self.shown.borrow_mut();
                shown.retain(|held| held.block != block || held.kind != kind);
                shown.push(ShownPresence { block, kind, value });
            }
            FromEditor::SendToPeers {
                to,
                channel,
                payload,
            } => self.peer_messages.borrow_mut().sent.push(SentToPeers {
                block,
                to,
                channel,
                payload,
            }),
            message => self.content_operations.borrow_mut().push((block, message)),
        }
    }

    pub fn request_content_resend(&self, block: Option<Uuid>) {
        let mut requests = self.resend_requests.borrow_mut();
        if !requests.contains(&block) {
            requests.push(block);
        }
        drop(requests);
        self.waker.wake();
    }

    #[cfg(test)]
    pub(crate) fn take_content_resend_requests(&self) -> Vec<Option<Uuid>> {
        std::mem::take(&mut self.resend_requests.borrow_mut())
    }

    pub fn watch_content(&self, block: Uuid, content_type: Uuid) {
        self.watched_content
            .borrow_mut()
            .insert(block, content_type);
    }

    pub fn content_of<C>(&self, block: Uuid) -> crate::LiveContent<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        self.watch_content(block, C::CONTENT_TYPE);
        crate::LiveContent::new(self.clone(), Some(block))
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

    pub fn send_to_peers(
        &self,
        block: Option<Uuid>,
        to: Option<u64>,
        channel: Uuid,
        payload: Vec<u8>,
    ) {
        self.peer_messages.borrow_mut().sent.push(SentToPeers {
            block,
            to,
            channel,
            payload,
        });
        self.waker.wake();
    }

    pub fn take_sent_to_peers(&self) -> Vec<SentToPeers> {
        std::mem::take(&mut self.peer_messages.borrow_mut().sent)
    }

    pub fn push_peer_messages(&self, block: Option<Uuid>, messages: Vec<PeerMessage>) {
        if messages.is_empty() {
            return;
        }
        {
            let mut held = self.peer_messages.borrow_mut();
            for message in messages {
                held.count += 1;
                let sequence = held.count;
                let queue = held.received.entry(block).or_default();
                queue.push_back((sequence, message));
                if queue.len() > PEER_MESSAGE_LIMIT {
                    queue.pop_front();
                }
            }
        }
        self.push(Pushed::PeerMessages);
        self.waker.wake();
    }

    pub fn peer_messages_seen(&self) -> u64 {
        self.peer_messages.borrow().count
    }

    pub fn peer_messages_since(&self, block: Option<Uuid>, seen: u64) -> Vec<(u64, PeerMessage)> {
        self.peer_messages
            .borrow()
            .received
            .get(&block)
            .map(|queue| {
                queue
                    .iter()
                    .filter(|(sequence, _)| *sequence > seen)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
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
        self.framed_view().map(|view| view.rect)
    }

    pub fn view_scale(&self) -> Option<f32> {
        self.framed_view().map(|view| view.scale)
    }

    fn framed_view(&self) -> Option<View> {
        let shown = self.showing.get().map(|shown| shown.placement);
        matches!(
            shown,
            None | Some(
                ViewPlacement::Parent(ParentView::Main)
                    | ViewPlacement::Host(HostView::Display(_))
                    | ViewPlacement::Dock
            )
        )
        .then(|| self.view.get())
        .flatten()
    }

    pub fn pan_view(&self, delta: Vec2) {
        self.view_changes.borrow_mut().push(ViewChange::Pan(delta));
    }

    pub fn zoom_view(&self, factor: f32, anchor: Option<Pos2>) {
        self.view_changes
            .borrow_mut()
            .push(ViewChange::Zoom { factor, anchor });
    }

    pub fn keep_fitted(&self, area: Option<FitArea>) {
        self.view_changes
            .borrow_mut()
            .push(ViewChange::KeepFitted(area));
    }

    pub fn fit_view(&self) {
        self.view_changes.borrow_mut().push(ViewChange::Fit);
    }

    pub fn pan_drag_view(&self) {
        self.view_changes.borrow_mut().push(ViewChange::PanDrag);
    }

    pub fn drag(&self) -> Option<BlockDrag> {
        let view = self.shown_view()?;
        match self.drops.borrow().get(&view)?.dragged {
            Some(Dragged::Block(drag)) => Some(drag),
            _ => None,
        }
    }

    pub fn files(&self) -> Option<FileDrop> {
        let view = self.shown_view()?;
        match &self.drops.borrow().get(&view)?.dragged {
            Some(Dragged::Files(drop)) => Some(drop.clone()),
            _ => None,
        }
    }

    fn shown_view(&self) -> Option<ViewId> {
        self.showing.get().map(|shown| shown.id)
    }

    pub fn accept_drag(&self, accepted: bool) {
        self.drag_accepted.set(Some(accepted));
    }

    pub(crate) fn set_instance(&self, instance: EditorInstanceId) {
        self.instance.set(Some(instance));
    }

    pub fn instance(&self) -> Option<EditorInstanceId> {
        self.instance.get()
    }

    pub fn then<T: 'static>(
        &self,
        asked: impl std::future::Future<Output = Option<T>> + 'static,
        answered: impl FnOnce(T) + 'static,
    ) -> Task {
        let host = self.clone();
        let task = crate::asked::spawn(async move {
            if let Some(answer) = asked.await {
                answered(answer);
                host.changed();
            }
        });
        let mut tasks = self.tasks.borrow_mut();
        tasks.retain(|task| !task.is_finished());
        tasks.push(task.clone());
        task
    }

    pub(crate) fn cancel_tasks(&self) {
        for task in std::mem::take(&mut *self.tasks.borrow_mut()) {
            task.cancel();
        }
    }

    fn ask<R>(&self, ask: impl FnOnce() -> R) -> R {
        asking(self.instance.get(), ask)
    }

    pub fn list_revisions(&self, block: Uuid) -> Asked<Result<Vec<Revision>, Failure>> {
        Asked::of(self.ask(|| host_services::list_revisions(block.into_bytes())))
    }

    pub fn preview_revision(&self, block: Uuid, commit: [u8; 32]) -> Asked<Result<Uuid, Failure>> {
        Asked::of(self.ask(|| host_services::preview_revision(block.into_bytes(), commit)))
            .map(|previewed| previewed.map(Uuid::from_bytes))
    }

    pub fn restore_revision(&self, block: Uuid, commit: [u8; 32]) -> Asked<Result<(), Failure>> {
        Asked::of(self.ask(|| host_services::restore_revision(block.into_bytes(), commit)))
    }

    pub fn release_revision(&self, preview: Uuid) {
        self.ask(|| host_services::release_revision(preview.into_bytes()).detach());
    }

    pub fn pick_file(&self, filter: FileFilter) -> Asked<FilePick> {
        Asked::of(self.ask(|| host_services::pick_file(filter)))
    }

    pub fn save_file(&self, file: SavedFile) -> Asked<FileSave> {
        Asked::of(self.ask(|| host_services::save_file(file)))
    }

    pub fn export_block(&self, block_id: Uuid) -> Asked<FileSave> {
        Asked::of(self.ask(|| host_services::export_block(block_id.into_bytes())))
    }

    pub fn read_plain_file(&self, block_id: Uuid) -> Asked<Result<SavedFile, Failure>> {
        Asked::of(self.ask(|| host_services::read_plain_file(block_id.into_bytes())))
    }

    pub fn import_blocks(
        &self,
        parent: BlockParent,
    ) -> Asked<Result<Option<(Uuid, Uuid)>, Failure>> {
        Asked::of(self.ask(|| host_services::import_blocks(parent.encode()))).map(|imported| {
            imported.map(|imported| {
                imported.map(|imported| {
                    (
                        Uuid::from_bytes(imported.block_id),
                        Uuid::from_bytes(imported.block_type),
                    )
                })
            })
        })
    }

    pub fn copy_blocks(
        &self,
        source: Uuid,
        parent: BlockParent,
    ) -> Asked<Result<Option<(Uuid, Uuid)>, Failure>> {
        Asked::of(self.ask(|| host_services::copy_blocks(source.into_bytes(), parent.encode())))
            .map(|copied| {
                copied.map(|copied| {
                    copied.map(|copied| {
                        (
                            Uuid::from_bytes(copied.block_id),
                            Uuid::from_bytes(copied.block_type),
                        )
                    })
                })
            })
    }

    pub fn list_access(&self, block_id: Uuid) -> Asked<Result<AccessListing, Failure>> {
        Asked::of(self.ask(|| host_services::list_access(block_id.into_bytes())))
    }

    pub fn contact_link(
        &self,
        block_id: Uuid,
        access: AccessLevel,
    ) -> Asked<Result<String, Failure>> {
        Asked::of(self.ask(|| host_services::make_contact_link(block_id.into_bytes(), access)))
    }

    pub fn set_access(&self, block_id: Uuid, account: Uuid, access: AccessLevel) {
        self.ask(|| {
            host_services::set_access(block_id.into_bytes(), account.into_bytes(), access).detach()
        });
    }

    pub fn load_audio(&self, media_type: impl Into<String>, bytes: Vec<u8>) {
        self.audio_commands
            .borrow_mut()
            .push(AudioCommand::Load(AudioSource {
                media_type: media_type.into(),
                bytes,
            }));
    }

    pub fn play_audio(&self) {
        self.audio_commands.borrow_mut().push(AudioCommand::Toggle);
    }

    pub fn reset_audio(&self) {
        self.audio_commands.borrow_mut().push(AudioCommand::Reset);
    }

    pub fn audio(&self) -> AudioStatus {
        self.audio_status.borrow().clone()
    }

    pub(crate) fn take_audio_commands(&self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.audio_commands.borrow_mut())
    }

    pub fn set_audio(&self, status: AudioStatus) {
        *self.audio_status.borrow_mut() = status;
        self.push(Pushed::Audio);
    }

    pub fn fetch(&self, url: impl Into<String>) -> Asked<Result<Vec<u8>, Failure>> {
        let url = url.into();
        Asked::of(self.ask(|| host_services::fetch(url)))
    }

    pub fn list_data(&self) -> Asked<Result<Vec<String>, Failure>> {
        Asked::of(self.ask(host_services::list_data))
    }

    pub fn read_data(&self, path: impl Into<String>) -> Asked<Result<Vec<u8>, Failure>> {
        let path = path.into();
        Asked::of(self.ask(|| host_services::read_data(path)))
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
        let mut next = self.next_frame.borrow_mut();
        let held = next.entry(self.shown_view()).or_insert(delay);
        *held = (*held).min(delay);
    }

    pub fn take_frame_request(&self, view: ViewId) -> Option<Duration> {
        let mut next = self.next_frame.borrow_mut();
        let own = next.remove(&Some(view));
        let any = next.remove(&None);
        own.into_iter().chain(any).min()
    }

    pub fn chrome_shown(&self) -> bool {
        self.chrome.get().unwrap_or(true)
    }

    pub fn placed_content(&self) -> Option<Rect> {
        self.placed_content.get()
    }

    pub fn set_placed_content(&self, placed: Option<Rect>) {
        if self.placed_content.replace(placed) != placed {
            self.changed();
        }
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

    pub fn copy_image(&self, png: Vec<u8>) {
        self.copied_images.borrow_mut().push(png);
    }

    pub fn take_copied_images(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.copied_images.borrow_mut())
    }

    pub fn request_paste(&self) {
        self.paste_requested.set(true);
    }

    pub fn take_paste_request(&self) -> bool {
        self.paste_requested.take()
    }

    pub fn join_module(&self, module: &crate::Module) {
        *self.module.borrow_mut() = module.downgrade();
    }

    pub fn module(&self) -> Option<crate::Module> {
        self.module
            .borrow()
            .upgrade()
            .map(crate::Module::from_state)
    }

    pub fn handle_views(&self, handler: Box<dyn crate::ViewHandler>) {
        self.module()
            .expect("an editor host belongs to a module before a handler registers")
            .handle(handler);
    }

    pub fn handles_views(&self) -> bool {
        self.module().is_some_and(|module| module.handles())
    }

    pub fn settings_draft(&self) -> Option<crate::SettingsDraft> {
        self.settings_draft.borrow().clone()
    }

    pub fn set_settings_draft(&self, draft: Vec<u8>) {
        if let Some(open) = self.settings_draft.borrow_mut().as_mut() {
            open.draft = draft;
        }
    }

    pub(crate) fn open_settings_draft(&self, draft: Option<crate::SettingsDraft>) {
        *self.settings_draft.borrow_mut() = draft;
    }

    pub(crate) fn close_settings_draft(&self) -> Option<Vec<u8>> {
        self.settings_draft
            .borrow_mut()
            .take()
            .map(|settings| settings.draft)
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

    pub fn set_menu(&self, view: ViewId, entries: Vec<MenuEntry>) {
        let mut menus = self.menus.borrow_mut();
        if menus
            .get(&view)
            .map_or(entries.is_empty(), |held| *held == entries)
        {
            return;
        }
        match entries.is_empty() {
            true => menus.remove(&view),
            false => menus.insert(view, entries),
        };
        drop(menus);
        self.changed();
    }

    pub(crate) fn menus(&self) -> std::collections::BTreeMap<ViewId, Vec<MenuEntry>> {
        self.menus.borrow().clone()
    }

    pub fn frame_anchor(&self) -> Option<Rect> {
        self.frame_anchor.get()
    }

    pub(crate) fn set_frame_anchor(&self, anchor: Option<Rect>) {
        if self.spec_anchor.replace(Some(anchor)) == Some(anchor) {
            return;
        }
        if self.frame_anchor.replace(anchor) != anchor {
            self.changed();
        }
    }

    pub(crate) fn push_menu_pick(&self, view: ViewId, id: String, anchor: Option<Rect>) {
        if let Some(anchor) = anchor
            && self.frame_anchor.replace(Some(anchor)) != Some(anchor)
        {
            self.changed();
        }
        self.menu_picks.borrow_mut().push((view, id));
        match view.is_main() {
            true => self.changed(),
            false => self.push(Pushed::Docks),
        }
    }

    pub fn take_menu_picks(&self, view: ViewId) -> Vec<String> {
        let mut taken = Vec::new();
        self.menu_picks.borrow_mut().retain(|(held, id)| {
            let here = *held == view;
            if here {
                taken.push(id.clone());
            }
            !here
        });
        taken
    }

    pub fn take_view_menu_picks(&self) -> Vec<(ViewId, String)> {
        let mut taken = Vec::new();
        self.menu_picks.borrow_mut().retain(|(held, id)| {
            let here = !held.is_main();
            if here {
                taken.push((*held, id.clone()));
            }
            !here
        });
        taken
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
        let view = self.shown_view().unwrap_or(ViewId::MAIN);
        let mut children = self.children.borrow_mut();
        let placing = children.placing.entry(view).or_default();
        let after = placing.placements.len() as u32;
        placing.occluders.push(Occluder { after, rect });
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
        corner_radius: f32,
        intrinsic: Option<Vec2>,
        follows_view: bool,
        dialog: Option<DialogFrame>,
        dock_tab: Option<u64>,
        docks: bool,
        anchor: Option<Rect>,
    ) -> ChildId {
        let view = self.shown_view().unwrap_or(ViewId::MAIN);
        let placed = rect;
        let clip =
            clip.intersect(geometry::Rotation::new(placed.center(), rotation).bounds(placed));
        let docks = dock_tab.filter(|_| docks && matches!(content, ChildContent::Block { .. }));
        if docks.is_some() {
            self.mark_docks_used();
        }
        let mut children = self.children.borrow_mut();
        let child = children.identify(view, dock_tab, Identity::of(&content));
        children
            .placing
            .entry(view)
            .or_default()
            .placements
            .push(ChildPlacement {
                child,
                content,
                rect: placed,
                clip,
                own_frame,
                top_bar,
                corner_radius: corner_radius.max(0.0),
                layer,
                mode,
                intrinsic: intrinsic.map(|size| Size {
                    width: size.x.max(0.0),
                    height: size.y.max(0.0),
                }),
                rotation,
                opacity: opacity.clamp(0.0, 1.0),
                follows_view,
                dialog,
                docks,
                anchor,
            });
        child
    }

    pub fn child_status(&self, child: ChildId) -> Option<ChildStatus> {
        self.child_statuses.borrow().get(&child).cloned()
    }

    pub fn handles_dismiss(&self, handles: bool) {
        if self.handles_dismiss.get() == handles {
            return;
        }
        self.handles_dismiss.set(handles);
        self.handles_dismiss_changed.set(true);
        self.changed();
    }

    pub fn close_dialog(&self, answer: DialogAnswer) {
        self.dialog_closes.borrow_mut().push(answer);
        self.changed();
    }

    pub fn dismiss_child(&self, child: ChildId) {
        self.child_dismissals.borrow_mut().push(child);
        self.changed();
    }

    pub(crate) fn request_dismiss(&self) {
        self.dismiss_requested.set(true);
        self.changed();
    }

    pub fn take_dismiss_request(&self) -> bool {
        self.dismiss_requested.take()
    }

    pub fn take_opens(&self) -> Vec<OpenRequest> {
        std::mem::take(&mut self.opens.borrow_mut())
    }

    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        *self.block_types.borrow_mut() = catalog;
        self.block_types_known.set(true);
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

    pub fn set_organization_id(&self, organization: Uuid) {
        self.organization.set(organization);
    }

    pub fn organization_id(&self) -> Uuid {
        self.organization.get()
    }

    pub fn graph(&self) -> crate::graph::BlockGraph {
        crate::graph::BlockGraph {
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

    pub fn fail_blocks(&self, query: crate::BlockQuery, error: String) {
        self.graph.set_failure(query, error);
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
        let view = self.shown_view().unwrap_or(ViewId::MAIN);
        self.drops.borrow_mut().entry(view).or_default().content = Some(rect);
    }

    pub(crate) fn take_content(&self, view: ViewId) -> Option<Rect> {
        self.drops.borrow_mut().get_mut(&view)?.content.take()
    }

    pub fn take_view_changes(&self) -> Vec<ViewChange> {
        std::mem::take(&mut self.view_changes.borrow_mut())
    }

    pub(crate) fn set_drag(&self, view: ViewId, dragged: Option<Dragged>) {
        self.drops.borrow_mut().entry(view).or_default().dragged = dragged;
        self.changed();
    }

    pub fn take_drag_accepted(&self) -> Option<bool> {
        self.drag_accepted.take()
    }

    pub fn in_view<R>(&self, view: block_plugin_api::View, within: impl FnOnce() -> R) -> R {
        let held = self.showing.replace(Some(view));
        let done = within();
        self.showing.set(held);
        done
    }

    pub(crate) fn begin_view(&self, view: ViewId) {
        self.children
            .borrow_mut()
            .placing
            .insert(view, Placing::default());
    }

    pub(crate) fn end_view(&self, view: ViewId) -> (Vec<ChildPlacement>, Vec<Occluder>) {
        let hosting = self.hosting_children();
        let mut children = self.children.borrow_mut();
        let placing = children.placing.remove(&view).unwrap_or_default();
        let gone: Vec<ChildKey> = children
            .identities
            .iter()
            .filter(|(key, child)| {
                children.placed_in(key.0, view)
                    && !placing.used.contains(key)
                    && !hosting.contains(child)
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in gone {
            children.identities.remove(&key);
        }
        (placing.placements, placing.occluders)
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

    pub fn take_child_bar_actions(&self, child: ChildId) -> Vec<BarAction> {
        self.child_bars
            .borrow_mut()
            .remove(&child)
            .unwrap_or_default()
    }

    pub fn take_child_focus(&self, child: ChildId) -> bool {
        self.child_focuses.borrow_mut().remove(&child)
    }

    pub(crate) fn push_child_focus(&self, child: ChildId) {
        self.child_focuses.borrow_mut().insert(child);
        self.changed();
    }

    pub(crate) fn push_child_bar_action(&self, child: ChildId, action: BarAction) {
        self.child_bars
            .borrow_mut()
            .entry(child)
            .or_default()
            .push(action);
        self.changed();
    }

    pub(crate) fn take_present_requests(&self) -> Vec<bool> {
        std::mem::take(&mut self.present_requests.borrow_mut())
    }

    pub(crate) fn take_handles_dismiss(&self) -> Option<bool> {
        self.handles_dismiss_changed
            .take()
            .then(|| self.handles_dismiss.get())
    }

    pub(crate) fn take_dialog_closes(&self) -> Vec<DialogAnswer> {
        std::mem::take(&mut self.dialog_closes.borrow_mut())
    }

    pub(crate) fn take_child_dismissals(&self) -> Vec<ChildId> {
        std::mem::take(&mut self.child_dismissals.borrow_mut())
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

#[cfg(test)]
mod tests;
