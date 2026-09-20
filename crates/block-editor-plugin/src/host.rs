use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

use block_plugin_api::{
    AccessLevel, ArtifactAction, AudioCommand, AudioStatus, BlockCommand, BlockLocation, BlockPick,
    ChildId, ChildLayer, ChildMode, ChildPlacement, ChildRect, ChildStatus, ClipboardImage,
    EditorBand, EditorCapabilities, EditorRegion, FetchResult, FilePick, InteractionMode, Occluder,
    PerformanceMeasurement, ResizeMode, ViewChange, WebViewCommand, WebViewEvent,
};
pub use block_plugin_api::{BlockFilter, FileFilter};
use block_ui::BlockCatalog;
use eframe::egui;
use uuid::Uuid;

pub type WebViewPlacement = (EditorRegion, Option<ChildRect>);

#[derive(Clone, Copy)]
pub struct BlockDrag {
    pub position: egui::Pos2,
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub dropped: bool,
}

#[derive(Clone)]
pub struct FileDrop {
    pub position: egui::Pos2,
    pub files: Vec<PickedFile>,
    pub dropped: bool,
}

pub type OpenRequest = (Uuid, Uuid, Option<Uuid>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShowRequest {
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub via: Option<Uuid>,
    pub from: Option<Uuid>,
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BlockSource {
    Root,
    Orphaned,
    Block(Uuid),
}

impl BlockSource {
    fn encode(self) -> BlockLocation {
        match self {
            Self::Root => BlockLocation::Root,
            Self::Orphaned => BlockLocation::Orphaned,
            Self::Block(id) => BlockLocation::Block(id.into_bytes()),
        }
    }
}

#[derive(Clone, Copy)]
pub struct PickedBlock {
    pub id: Uuid,
    pub block_type: Uuid,
    pub linked: bool,
}

#[derive(Clone, Default)]
pub struct FocusedBlock {
    pub block_id: Option<Uuid>,
    pub block_type: Uuid,
    pub via: Vec<Uuid>,
}

impl FocusedBlock {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
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

#[derive(Clone)]
pub struct PickedFile {
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Default)]
pub struct Waker(Option<Arc<dyn Fn() + Send + Sync>>);

impl Waker {
    pub fn wake(&self) {
        if let Some(wake) = &self.0 {
            wake();
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Some(Arc::new(wake)))
    }
}
pub struct Task<T> {
    receiver: Receiver<T>,
    result: Option<T>,
    done: bool,
}

impl<T: Send + 'static> Task<T> {
    fn spawn(waker: &Waker, future: impl std::future::Future<Output = T> + Send + 'static) -> Self {
        let (sender, receiver) = mpsc::channel();
        let waker = waker.clone();
        block_client::spawn(async move {
            let _ = sender.send(future.await);
            waker.wake();
        });
        Self {
            receiver,
            result: None,
            done: false,
        }
    }
}

impl<T> Task<T> {
    pub fn poll(&mut self) -> Option<&T> {
        if !self.done {
            match self.receiver.try_recv() {
                Ok(result) => {
                    self.result = Some(result);
                    self.done = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.done = true,
            }
        }
        self.result.as_ref()
    }

    pub fn take(&mut self) -> Option<T> {
        self.poll();
        self.result.take()
    }

    pub fn finished(&self) -> bool {
        self.done
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
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
    origin: egui::Vec2,
}

type ChildKey = (EditorRegion, Uuid, u32);

#[derive(Default)]
struct Children {
    placements: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    ordinals: HashMap<Uuid, u32>,
    identities: HashMap<ChildKey, ChildId>,
    used: Vec<ChildKey>,
    next: u64,
}

impl Children {
    fn identify(&mut self, region: EditorRegion, block_id: Uuid) -> ChildId {
        let ordinal = {
            let ordinal = self.ordinals.entry(block_id).or_default();
            let current = *ordinal;
            *ordinal += 1;
            current
        };
        let key = (region, block_id, ordinal);
        let child = match self.identities.get(&key) {
            Some(child) => *child,
            None => {
                self.next += 1;
                let child = ChildId(self.next);
                self.identities.insert(key, child);
                child
            }
        };
        self.used.push(key);
        child
    }
}

pub struct ChildHandle {
    host: EditorHost,
    index: usize,
    child: ChildId,
    painter: egui::Painter,
    shape: egui::layers::ShapeIdx,
    rect: egui::Rect,
    status: Option<ChildStatus>,
    pub response: egui::Response,
}

impl ChildHandle {
    pub fn id(&self) -> ChildId {
        self.child
    }

    pub fn rect(&self) -> egui::Rect {
        self.rect
    }

    pub fn available(&self) -> bool {
        self.status.as_ref().is_some_and(|status| status.available)
    }

    pub fn hovered(&self) -> bool {
        self.status.as_ref().is_some_and(|status| status.hovered)
    }

    pub fn active(&self) -> bool {
        self.status.as_ref().is_some_and(|status| status.active)
    }

    pub fn reported(&self) -> bool {
        self.status.is_some()
    }

    pub fn interaction(&self) -> InteractionMode {
        self.status
            .as_ref()
            .map_or(InteractionMode::Preview, |status| status.interaction)
    }

    pub fn capabilities(&self) -> EditorCapabilities {
        self.status
            .as_ref()
            .map_or_else(EditorCapabilities::default, |status| status.capabilities)
    }

    pub fn resize(&self) -> ResizeMode {
        self.status
            .as_ref()
            .map_or(ResizeMode::None, |status| status.resize)
    }

    pub fn set_intrinsic_size(&self, size: egui::Vec2) {
        self.host.update_child(self.index, |placement| {
            placement.intrinsic_width = size.x.max(0.0);
            placement.intrinsic_height = size.y.max(0.0);
        });
    }

    pub fn take_view_changes(&self) -> Vec<ViewChange> {
        self.host.take_child_view_changes(self.child)
    }

    pub fn set_rotation(&self, radians: f32) {
        self.host.update_child(self.index, |placement| {
            placement.rotation = radians;
        });
    }

    pub fn set_opacity(&self, opacity: f32) {
        self.host.update_child(self.index, |placement| {
            placement.opacity = opacity.clamp(0.0, 1.0);
        });
    }

    pub fn error(&self) -> Option<&str> {
        self.status.as_ref()?.error.as_deref()
    }

    pub fn intrinsic_size(&self) -> Option<egui::Vec2> {
        let status = self.status.as_ref()?;
        (status.intrinsic_width > 0.0 && status.intrinsic_height > 0.0)
            .then(|| egui::vec2(status.intrinsic_width, status.intrinsic_height))
    }

    pub fn aspect_ratio(&self) -> Option<f32> {
        let status = self.status.as_ref()?;
        (status.aspect_ratio > 0.0).then_some(status.aspect_ratio)
    }

    pub fn set_mode(&self, mode: ChildMode) {
        self.host.update_child(self.index, |placement| {
            placement.mode = mode;
        });
    }

    pub fn activate(&self) {
        self.set_mode(ChildMode::Active);
    }

    pub fn keep_active(&self) {
        self.set_mode(ChildMode::Live);
    }

    pub fn own_frame(&self) {
        self.host.update_child(self.index, |placement| {
            placement.own_frame = true;
        });
    }

    pub fn set_corner_radius(&self, radius: f32) {
        let layer = self.host.update_child(self.index, |placement| {
            placement.corner_radius = radius;
            placement.layer
        });
        let Some(layer) = layer else {
            return;
        };
        self.painter
            .set(self.shape, self.host.child_shape(self.rect, radius, layer));
    }
}

#[derive(Clone, Copy)]
struct View {
    rect: egui::Rect,
    scale: f32,
}

#[derive(Clone, Copy)]
struct BeuiFrame {
    ratio: f32,
    pixels_per_point: f32,
    chrome: bool,
    content: Option<beui::Rect>,
}

impl Default for BeuiFrame {
    fn default() -> Self {
        Self {
            ratio: 1.0,
            pixels_per_point: 1.0,
            chrome: true,
            content: None,
        }
    }
}

#[derive(Clone)]
pub struct BeuiView {
    host: EditorHost,
}

impl BeuiView {
    pub fn rect(&self) -> Option<beui::Rect> {
        let ratio = self.host.beui.get().ratio;
        self.host.view().map(|rect| beui_rect(rect, ratio))
    }

    pub fn scale(&self) -> f32 {
        self.host.view_scale().unwrap_or(1.0)
    }

    pub fn canvas(&self) -> Option<beui::reactive::CanvasView> {
        let scale = self.scale();
        self.rect()
            .map(|rect| beui::reactive::CanvasView::new(rect.min, scale))
    }

    pub fn pan(&self, delta: beui::Vec2) {
        let ratio = self.host.beui.get().ratio;
        self.host
            .pan_view(egui::vec2(delta.x / ratio, delta.y / ratio));
    }

    pub fn zoom(&self, factor: f32, anchor: Option<beui::Pos2>) {
        let ratio = self.host.beui.get().ratio;
        self.host.zoom_view(
            factor,
            anchor.map(|anchor| egui::pos2(anchor.x / ratio, anchor.y / ratio)),
        );
    }

    pub fn fit(&self) {
        self.host.fit_view();
    }

    pub fn set_content(&self, rect: beui::Rect) {
        let mut frame = self.host.beui.get();
        frame.content = Some(rect);
        self.host.beui.set(frame);
    }
}

fn swept(rect: egui::Rect, rotation: f32) -> egui::Rect {
    if rotation == 0.0 {
        return rect;
    }
    let center = rect.center();
    let (sin, cos) = rotation.sin_cos();
    let turned = |corner: egui::Pos2| {
        let offset = corner - center;
        center + egui::vec2(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos)
    };
    egui::Rect::from_points(&[
        turned(rect.left_top()),
        turned(rect.right_top()),
        turned(rect.right_bottom()),
        turned(rect.left_bottom()),
    ])
}

fn host_rect(rect: beui::Rect, ratio: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(rect.min.x / ratio, rect.min.y / ratio),
        egui::pos2(rect.max.x / ratio, rect.max.y / ratio),
    )
}

fn beui_rect(rect: egui::Rect, ratio: f32) -> beui::Rect {
    beui::Rect::from_min_max(
        beui::pos2(rect.min.x * ratio, rect.min.y * ratio),
        beui::pos2(rect.max.x * ratio, rect.max.y * ratio),
    )
}

#[derive(Clone, Default)]
pub struct EditorHost {
    waker: Waker,
    opens: Rc<RefCell<Vec<OpenRequest>>>,
    shows: Rc<RefCell<Vec<ShowRequest>>>,
    focused: Rc<RefCell<FocusedBlock>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    reported_focus: Rc<RefCell<Option<FocusedBlock>>>,
    artifacts: Rc<RefCell<HashMap<Uuid, ArtifactState>>>,
    watched_artifacts: Rc<RefCell<Vec<Uuid>>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    reported_artifacts: Rc<RefCell<Option<Vec<Uuid>>>>,
    block_drags: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
    block_commands: Rc<RefCell<Vec<(Uuid, BlockCommand)>>>,
    block_types: Rc<RefCell<Rc<BlockCatalog>>>,
    drag: Rc<Cell<Option<BlockDrag>>>,
    files: Rc<RefCell<Option<FileDrop>>>,
    drag_accepted: Rc<Cell<Option<bool>>>,
    picks: Rc<RefCell<Vec<(u64, FileFilter)>>>,
    picked: Rc<RefCell<HashMap<u64, FilePick>>>,
    next_pick: Rc<Cell<u64>>,
    editable: Rc<Cell<bool>>,
    client_id: Rc<Cell<Uuid>>,
    view: Rc<Cell<Option<View>>>,
    view_changes: Rc<RefCell<Vec<ViewChange>>>,
    creation_ready: Rc<Cell<bool>>,
    creation_changed: Rc<Cell<bool>>,
    performance: Arc<Mutex<Vec<PerformanceRecord>>>,
    region: Rc<Cell<Region>>,
    children: Rc<RefCell<Children>>,
    child_statuses: Rc<RefCell<HashMap<ChildId, ChildStatus>>>,
    block_picks: Rc<RefCell<Vec<(u64, BlockFilter)>>>,
    blocks_picked: Rc<RefCell<HashMap<u64, BlockPick>>>,
    next_block_pick: Rc<Cell<u64>>,
    audio_commands: Rc<RefCell<Vec<(Uuid, AudioCommand)>>>,
    audio_status: Rc<RefCell<AudioStatus>>,
    pastes: Rc<RefCell<Vec<u64>>>,
    pasted: Rc<RefCell<HashMap<u64, ClipboardImage>>>,
    next_paste: Rc<Cell<u64>>,
    fetches: Rc<RefCell<Vec<(u64, String)>>>,
    fetched: Rc<RefCell<HashMap<u64, FetchResult>>>,
    next_fetch: Rc<Cell<u64>>,
    web_view_placements: Rc<RefCell<Vec<WebViewPlacement>>>,
    web_view_commands: Rc<RefCell<Vec<WebViewCommand>>>,
    web_view_events: Rc<RefCell<Vec<WebViewEvent>>>,
    cursor_grabbed: Rc<Cell<bool>>,
    cursor_grab_changed: Rc<Cell<bool>>,
    presenting: Rc<Cell<bool>>,
    present_requests: Rc<RefCell<Vec<bool>>>,
    child_views: Rc<RefCell<HashMap<ChildId, Vec<ViewChange>>>>,
    hidden_bands: Rc<RefCell<HashSet<EditorBand>>>,
    beui: Rc<Cell<BeuiFrame>>,
    next_frame: Rc<Cell<Option<Duration>>>,
}

impl EditorHost {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            waker,
            ..Self::default()
        }
    }

    pub fn waker(&self) -> Waker {
        self.waker.clone()
    }

    pub fn spawn<T: Send + 'static>(
        &self,
        future: impl std::future::Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        Task::spawn(&self.waker, future)
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

    pub fn show_block(
        &self,
        block_id: Uuid,
        block_type: Uuid,
        via: Option<Uuid>,
        from: Option<Uuid>,
    ) {
        self.shows.borrow_mut().push(ShowRequest {
            block_id,
            block_type,
            via,
            from,
        });
    }

    pub fn focused_block(&self) -> FocusedBlock {
        self.focused.borrow().clone()
    }

    pub fn report_focus(&self, focused: FocusedBlock) {
        *self.focused.borrow_mut() = focused;
    }

    #[cfg(target_arch = "wasm32")]
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_artifact_watch(&self) -> Option<Vec<Uuid>> {
        let blocks = self.watched_artifacts.borrow().clone();
        let mut reported = self.reported_artifacts.borrow_mut();
        if reported.as_ref() == Some(&blocks) {
            return None;
        }
        *reported = Some(blocks.clone());
        Some(blocks)
    }

    pub fn set_artifacts(&self, states: Vec<ArtifactState>) {
        *self.artifacts.borrow_mut() = states
            .into_iter()
            .map(|state| (state.block_id, state))
            .collect();
    }

    pub fn regenerate_artifact(&self, block_id: Uuid) {
        self.artifact_command(block_id, ArtifactAction::Regenerate);
    }

    pub fn edit_artifact(&self, block_id: Uuid) {
        self.artifact_command(block_id, ArtifactAction::Settings);
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_focused_block(&self, focused: FocusedBlock) {
        *self.focused.borrow_mut() = focused;
    }

    pub fn drag_block(&self, block_id: Uuid, block_type: Uuid) {
        self.block_drags.borrow_mut().push((block_id, block_type));
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_block_drags(&self) -> Vec<(Uuid, Uuid)> {
        std::mem::take(&mut self.block_drags.borrow_mut())
    }

    pub fn share_block(&self, block_id: Uuid) {
        self.block_commands
            .borrow_mut()
            .push((block_id, BlockCommand::Share));
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
        source: BlockSource,
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
        source: BlockSource,
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_block_commands(&self) -> Vec<(Uuid, BlockCommand)> {
        std::mem::take(&mut self.block_commands.borrow_mut())
    }

    pub fn block_types(&self) -> Rc<BlockCatalog> {
        Rc::clone(&self.block_types.borrow())
    }

    pub fn editable(&self) -> bool {
        self.editable.get()
    }

    pub fn client_id(&self) -> Uuid {
        self.client_id.get()
    }

    pub fn view(&self) -> Option<egui::Rect> {
        let origin = self.region.get().origin;
        self.view.get().map(|view| view.rect.translate(origin))
    }

    pub fn view_scale(&self) -> Option<f32> {
        self.view.get().map(|view| view.scale)
    }

    pub fn pan_view(&self, delta: egui::Vec2) {
        self.view_changes.borrow_mut().push(ViewChange::Pan {
            x: delta.x,
            y: delta.y,
        });
    }

    pub fn zoom_view(&self, factor: f32, anchor: Option<egui::Pos2>) {
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_files(&self, drop: Option<FileDrop>) {
        *self.files.borrow_mut() = drop;
    }

    pub fn beui_drag(&self) -> Option<crate::editor::Drag> {
        let ratio = self.beui.get().ratio;
        self.drag().map(|drag| crate::editor::Drag {
            position: beui::pos2(drag.position.x * ratio, drag.position.y * ratio),
            block_id: drag.block_id,
            block_type: drag.block_type,
            dropped: drag.dropped,
        })
    }

    pub fn accept_drag(&self, accepted: bool) {
        self.drag_accepted.set(Some(accepted));
    }

    pub fn pick_file(&self, filter: FileFilter) -> u64 {
        let request = self.next_pick.get() + 1;
        self.next_pick.set(request);
        self.picks.borrow_mut().push((request, filter));
        request
    }

    pub fn take_pick(&self, request: u64) -> Option<FilePick> {
        self.picked.borrow_mut().remove(&request)
    }

    pub fn pick_block(&self, filter: BlockFilter) -> u64 {
        let request = self.next_block_pick.get() + 1;
        self.next_block_pick.set(request);
        self.block_picks.borrow_mut().push((request, filter));
        request
    }

    pub fn take_block_pick(&self, request: u64) -> Option<BlockPick> {
        self.blocks_picked.borrow_mut().remove(&request)
    }

    pub fn paste_image(&self) -> u64 {
        let request = self.next_paste.get() + 1;
        self.next_paste.set(request);
        self.pastes.borrow_mut().push(request);
        request
    }

    pub fn take_pasted_image(&self, request: u64) -> Option<ClipboardImage> {
        self.pasted.borrow_mut().remove(&request)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_pastes(&self) -> Vec<u64> {
        std::mem::take(&mut self.pastes.borrow_mut())
    }

    pub fn set_pasted_image(&self, request: u64, image: ClipboardImage) {
        self.pasted.borrow_mut().insert(request, image);
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_audio_commands(&self) -> Vec<(Uuid, AudioCommand)> {
        std::mem::take(&mut self.audio_commands.borrow_mut())
    }

    pub fn set_audio(&self, status: AudioStatus) {
        *self.audio_status.borrow_mut() = status;
    }

    pub fn fetch(&self, url: impl Into<String>) -> u64 {
        let request = self.next_fetch.get() + 1;
        self.next_fetch.set(request);
        self.fetches.borrow_mut().push((request, url.into()));
        request
    }

    pub fn take_fetch(&self, request: u64) -> Option<FetchResult> {
        self.fetched.borrow_mut().remove(&request)
    }

    pub fn take_fetches(&self) -> Vec<(u64, String)> {
        std::mem::take(&mut self.fetches.borrow_mut())
    }

    pub fn set_fetched(&self, request: u64, result: FetchResult) {
        self.fetched.borrow_mut().insert(request, result);
    }

    pub fn place_web_view(&self, rect: Option<egui::Rect>) {
        let state = self.region.get();
        let region = state.region.unwrap_or(EditorRegion::Frame);
        let rect = rect.map(|rect| child_rect(rect.translate(-state.origin)));
        self.web_view_placements.borrow_mut().push((region, rect));
    }

    pub fn place_beui_web_view(&self, rect: Option<beui::Rect>) {
        let ratio = self.beui.get().ratio;
        self.place_web_view(rect.map(|rect| host_rect(rect, ratio)));
    }

    pub fn open_web_view(&self, url: impl Into<String>) {
        self.command_web_view(WebViewCommand::Open(url.into()));
    }

    pub fn load_web_view(&self, url: impl Into<String>) {
        self.command_web_view(WebViewCommand::Load(url.into()));
    }

    pub fn reload_web_view(&self) {
        self.command_web_view(WebViewCommand::Reload);
    }

    pub fn close_web_view(&self) {
        self.command_web_view(WebViewCommand::Close);
    }

    pub fn focus_app(&self) {
        self.command_web_view(WebViewCommand::FocusApp);
    }

    fn command_web_view(&self, command: WebViewCommand) {
        self.web_view_commands.borrow_mut().push(command);
    }

    pub fn take_web_view_events(&self) -> Vec<WebViewEvent> {
        std::mem::take(&mut self.web_view_events.borrow_mut())
    }

    pub fn take_web_view_placements(&self) -> Vec<WebViewPlacement> {
        std::mem::take(&mut self.web_view_placements.borrow_mut())
    }

    pub fn take_web_view_commands(&self) -> Vec<WebViewCommand> {
        std::mem::take(&mut self.web_view_commands.borrow_mut())
    }

    pub fn push_web_view_event(&self, event: WebViewEvent) {
        self.web_view_events.borrow_mut().push(event);
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

    pub fn child(&self, ui: &mut egui::Ui, block_id: Uuid, block_type: Uuid) -> ChildHandle {
        let size = ui.available_size_before_wrap();
        self.place_child(ui, size, block_id, block_type, ChildLayer::Below)
    }

    pub fn child_sized(
        &self,
        ui: &mut egui::Ui,
        size: egui::Vec2,
        block_id: Uuid,
        block_type: Uuid,
    ) -> ChildHandle {
        self.place_child(ui, size, block_id, block_type, ChildLayer::Below)
    }

    pub fn child_above(&self, ui: &mut egui::Ui, block_id: Uuid, block_type: Uuid) -> ChildHandle {
        let size = ui.available_size_before_wrap();
        self.place_child(ui, size, block_id, block_type, ChildLayer::Above)
    }

    pub fn show_band(&self, band: EditorBand, shown: bool) {
        let mut hidden = self.hidden_bands.borrow_mut();
        match shown {
            true => hidden.remove(&band),
            false => hidden.insert(band),
        };
    }

    pub fn band_shown(&self, band: EditorBand) -> bool {
        !self.hidden_bands.borrow().contains(&band)
    }

    pub fn beui_view(&self) -> BeuiView {
        BeuiView { host: self.clone() }
    }

    pub fn chrome_shown(&self) -> bool {
        self.beui.get().chrome
    }

    pub fn set_chrome_shown(&self, chrome: bool) {
        let mut frame = self.beui.get();
        frame.chrome = chrome;
        self.beui.set(frame);
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

    pub fn occlude(&self, rect: egui::Rect) {
        let origin = self.region.get().origin;
        let mut children = self.children.borrow_mut();
        let after = children.placements.len() as u32;
        children.occluders.push(Occluder {
            after,
            rect: child_rect(rect.translate(-origin)),
        });
    }

    pub fn place_beui_child(
        &self,
        block_id: Uuid,
        block_type: Uuid,
        rect: beui::Rect,
        clip: beui::Rect,
        mode: ChildMode,
        layer: ChildLayer,
        own_frame: bool,
        rotation: f32,
        opacity: f32,
        intrinsic: Option<beui::Vec2>,
    ) -> ChildId {
        let ratio = self.beui.get().ratio;
        let state = self.region.get();
        let rect = host_rect(rect, ratio);
        let clip = host_rect(clip, ratio).intersect(swept(rect, rotation));
        let mut children = self.children.borrow_mut();
        let child = children.identify(state.region.unwrap_or(EditorRegion::Frame), block_id);
        children.placements.push(ChildPlacement {
            child,
            block_id: block_id.into_bytes(),
            block_type: block_type.into_bytes(),
            rect: child_rect(rect.translate(-state.origin)),
            clip: child_rect(clip.translate(-state.origin)),
            own_frame,
            corner_radius: 0.0,
            layer,
            mode,
            intrinsic_width: intrinsic.map_or(0.0, |size| size.x.max(0.0)),
            intrinsic_height: intrinsic.map_or(0.0, |size| size.y.max(0.0)),
            rotation,
            opacity: opacity.clamp(0.0, 1.0),
        });
        child
    }

    pub fn child_status(&self, child: ChildId) -> Option<ChildStatus> {
        self.child_statuses.borrow().get(&child).cloned()
    }

    fn place_child(
        &self,
        ui: &mut egui::Ui,
        size: egui::Vec2,
        block_id: Uuid,
        block_type: Uuid,
        layer: ChildLayer,
    ) -> ChildHandle {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let painter = ui.painter().clone();
        let state = self.region.get();
        let region = state.region.unwrap_or(EditorRegion::Frame);
        let mut children = self.children.borrow_mut();
        let child = children.identify(region, block_id);
        let shape = painter.add(self.child_shape(rect, 0.0, layer));
        let index = children.placements.len();
        children.placements.push(ChildPlacement {
            child,
            block_id: block_id.into_bytes(),
            block_type: block_type.into_bytes(),
            rect: child_rect(rect.translate(-state.origin)),
            clip: child_rect(ui.clip_rect().intersect(rect).translate(-state.origin)),
            own_frame: false,
            corner_radius: 0.0,
            layer,
            mode: ChildMode::Passive,
            intrinsic_width: 0.0,
            intrinsic_height: 0.0,
            rotation: 0.0,
            opacity: 1.0,
        });
        drop(children);
        ChildHandle {
            host: self.clone(),
            index,
            child,
            painter,
            shape,
            rect,
            status: self.child_statuses.borrow().get(&child).cloned(),
            response,
        }
    }

    fn child_shape(&self, rect: egui::Rect, radius: f32, layer: ChildLayer) -> egui::Shape {
        match layer {
            ChildLayer::Below => punch_shape(rect, radius),
            ChildLayer::Above => egui::Shape::Noop,
        }
    }

    fn update_child<T>(
        &self,
        index: usize,
        edit: impl FnOnce(&mut ChildPlacement) -> T,
    ) -> Option<T> {
        Some(edit(self.children.borrow_mut().placements.get_mut(index)?))
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        *self.block_types.borrow_mut() = catalog;
    }

    pub fn set_client_id(&self, client_id: Uuid) {
        self.client_id.set(client_id);
    }

    pub fn set_editable(&self, editable: bool) {
        self.editable.set(editable);
    }

    pub fn set_view(&self, view: egui::Rect, scale: f32) {
        self.view.set(Some(View { rect: view, scale }));
    }

    pub fn set_beui_view(&self, view: beui::Rect, scale: f32) {
        let ratio = self.beui.get().ratio;
        let origin = self.region.get().origin;
        let rect = egui::Rect::from_min_max(
            egui::pos2(view.min.x / ratio, view.min.y / ratio),
            egui::pos2(view.max.x / ratio, view.max.y / ratio),
        );
        self.set_view(rect.translate(-origin), scale);
    }

    pub fn begin_beui_frame(&self, ratio: f32, pixels_per_point: f32, chrome: bool) {
        self.beui.set(BeuiFrame {
            ratio,
            pixels_per_point,
            chrome,
            content: None,
        });
    }

    pub fn beui_pixels_per_point(&self) -> f32 {
        self.beui.get().pixels_per_point
    }

    pub fn take_beui_content(&self) -> Option<beui::Rect> {
        let mut frame = self.beui.get();
        let content = frame.content.take();
        self.beui.set(frame);
        content
    }

    pub fn take_view_changes(&self) -> Vec<ViewChange> {
        std::mem::take(&mut self.view_changes.borrow_mut())
    }

    pub fn set_drag(&self, drag: Option<BlockDrag>) {
        self.drag.set(drag);
    }

    pub fn set_beui_drag(&self, drag: Option<crate::editor::Drag>) {
        let ratio = self.beui.get().ratio;
        self.drag.set(drag.map(|drag| BlockDrag {
            position: egui::pos2(drag.position.x / ratio, drag.position.y / ratio),
            block_id: drag.block_id,
            block_type: drag.block_type,
            dropped: drag.dropped,
        }));
    }

    pub fn take_drag_accepted(&self) -> Option<bool> {
        self.drag_accepted.take()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_picks(&self) -> Vec<(u64, FileFilter)> {
        std::mem::take(&mut self.picks.borrow_mut())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_pick(&self, request: u64, pick: FilePick) {
        self.picked.borrow_mut().insert(request, pick);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_block_picks(&self) -> Vec<(u64, BlockFilter)> {
        std::mem::take(&mut self.block_picks.borrow_mut())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_block_pick(&self, request: u64, pick: BlockPick) {
        self.blocks_picked.borrow_mut().insert(request, pick);
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn begin_region(&self, region: EditorRegion, origin: egui::Vec2) {
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
    }

    pub fn retain_child_statuses(&self, live: &[ChildId]) {
        self.child_statuses
            .borrow_mut()
            .retain(|child, _| live.contains(child));
    }

    pub fn set_presenting(&self, presenting: bool) {
        self.presenting.set(presenting);
    }

    fn take_child_view_changes(&self, child: ChildId) -> Vec<ViewChange> {
        self.child_views
            .borrow_mut()
            .remove(&child)
            .unwrap_or_default()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn push_child_view_change(&self, child: ChildId, change: ViewChange) {
        self.child_views
            .borrow_mut()
            .entry(child)
            .or_default()
            .push(change);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_present_requests(&self) -> Vec<bool> {
        std::mem::take(&mut self.present_requests.borrow_mut())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_creation_ready(&self) -> Option<bool> {
        self.creation_changed
            .take()
            .then(|| self.creation_ready.get())
    }

    #[cfg(target_arch = "wasm32")]
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
pub struct FilePicker {
    request: Option<u64>,
}

impl FilePicker {
    pub fn open(&mut self, host: &EditorHost, filter: FileFilter) {
        self.request = Some(host.pick_file(filter));
    }

    pub fn is_open(&self) -> bool {
        self.request.is_some()
    }

    pub fn poll(&mut self, host: &EditorHost) -> Option<Result<PickedFile, String>> {
        let pick = host.take_pick(self.request?)?;
        self.request = None;
        match pick {
            FilePick::Chosen { name, data } => Some(Ok(PickedFile { name, data })),
            FilePick::Cancelled => None,
            FilePick::Failed(error) => Some(Err(error)),
        }
    }
}

#[derive(Default)]
pub struct ImagePaster {
    request: Option<u64>,
}

impl ImagePaster {
    pub fn poll(&mut self, ui: &egui::Ui, host: &EditorHost, enabled: bool) -> Option<PastedImage> {
        self.settle(host, enabled && pasted(ui))
    }

    pub fn paste(&mut self, host: &EditorHost, asked: bool) -> Option<PastedImage> {
        self.settle(host, asked)
    }

    fn settle(&mut self, host: &EditorHost, asked: bool) -> Option<PastedImage> {
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

fn pasted(ui: &egui::Ui) -> bool {
    ui.input(|input| {
        input
            .raw
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Paste(_)))
            || (input.modifiers.command && input.key_pressed(egui::Key::V))
    })
}

fn child_rect(rect: egui::Rect) -> ChildRect {
    ChildRect {
        x: rect.min.x,
        y: rect.min.y,
        width: rect.width().max(0.0),
        height: rect.height().max(0.0),
    }
}

fn punch_shape(rect: egui::Rect, radius: f32) -> egui::Shape {
    #[cfg(target_arch = "wasm32")]
    {
        crate::panes::punch(rect, radius)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (rect, radius);
        egui::Shape::Noop
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
            } => Some(Ok(PickedBlock {
                id: Uuid::from_bytes(block_id),
                block_type: Uuid::from_bytes(block_type),
                linked,
            })),
            BlockPick::Cancelled => None,
            BlockPick::Failed(error) => Some(Err(error)),
        }
    }
}

#[cfg(test)]
mod tests;
