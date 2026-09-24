use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
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
    EditorRegion, FetchResult, FilePick, HostReply, HostRequest, Occluder, PerformanceMeasurement,
    Size, ViewChange, WebViewCommand, WebViewEvent,
};
pub use block_plugin_api::{BlockFilter, FileFilter};
use block_ui::BlockCatalog;
use uuid::Uuid;

pub type WebViewPlacement = (EditorRegion, Option<ChildRect>);

#[derive(Clone, Copy)]
pub struct BlockDrag {
    pub position: beui::Pos2,
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub dropped: bool,
}

#[derive(Clone)]
pub struct HostContent {
    pub content_type: Uuid,
    pub bytes: Vec<u8>,
    pub applied: u64,
}

type ContentOperation = (Option<Uuid>, Vec<u8>);

#[derive(Clone)]
pub enum ContentUpdate {
    Snapshot(HostContent),
    Operations(Vec<(Vec<u8>, bool)>),
}

#[derive(Clone)]
pub struct FileDrop {
    pub position: beui::Pos2,
    pub files: Vec<PickedFile>,
    pub dropped: bool,
}

pub type OpenRequest = (Uuid, Uuid, Option<Uuid>);

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
    origin: beui::Vec2,
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

#[derive(Clone, Copy)]
struct View {
    rect: beui::Rect,
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
            .pan_view(beui::vec2(delta.x / ratio, delta.y / ratio));
    }

    pub fn zoom(&self, factor: f32, anchor: Option<beui::Pos2>) {
        let ratio = self.host.beui.get().ratio;
        self.host.zoom_view(
            factor,
            anchor.map(|anchor| beui::pos2(anchor.x / ratio, anchor.y / ratio)),
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

fn swept(rect: beui::Rect, rotation: f32) -> beui::Rect {
    if rotation == 0.0 {
        return rect;
    }
    let center = rect.center();
    let (sin, cos) = rotation.sin_cos();
    let turned = |corner: beui::Pos2| {
        let offset = corner - center;
        center
            + beui::vec2(
                offset.x * cos - offset.y * sin,
                offset.x * sin + offset.y * cos,
            )
    };
    beui::Rect::from_points(&[
        turned(rect.left_top()),
        turned(rect.right_top()),
        turned(rect.right_bottom()),
        turned(rect.left_bottom()),
    ])
}

fn host_rect(rect: beui::Rect, ratio: f32) -> beui::Rect {
    scaled(rect, ratio.recip())
}

fn beui_rect(rect: beui::Rect, ratio: f32) -> beui::Rect {
    scaled(rect, ratio)
}

fn scaled(rect: beui::Rect, ratio: f32) -> beui::Rect {
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
    histories: Rc<RefCell<HashMap<Uuid, BlockHistory>>>,
    watched_history: Rc<RefCell<Vec<Uuid>>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    reported_history: Rc<RefCell<Option<Vec<Uuid>>>>,
    block_drags: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
    block_commands: Rc<RefCell<Vec<(Uuid, BlockCommand)>>>,
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
    web_view_placements: Rc<RefCell<Vec<WebViewPlacement>>>,
    web_view_commands: Rc<RefCell<Vec<WebViewCommand>>>,
    web_view_events: Rc<RefCell<Vec<WebViewEvent>>>,
    cursor_grabbed: Rc<Cell<bool>>,
    cursor_grab_changed: Rc<Cell<bool>>,
    presenting: Rc<Cell<bool>>,
    present_requests: Rc<RefCell<Vec<bool>>>,
    child_views: Rc<RefCell<HashMap<ChildId, Vec<ViewChange>>>>,
    beui: Rc<Cell<BeuiFrame>>,
    next_frame: Rc<Cell<Option<Duration>>>,
    content_updates: Rc<RefCell<HashMap<Option<Uuid>, Vec<ContentUpdate>>>>,
    content_operations: Rc<RefCell<Vec<ContentOperation>>>,
    watched_content: Rc<RefCell<std::collections::BTreeMap<Uuid, Uuid>>>,
    seeded: Rc<RefCell<Vec<SeededContent>>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    reported_content: Rc<RefCell<Option<std::collections::BTreeMap<Uuid, Uuid>>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeededContent {
    pub block: Uuid,
    pub content_type: Uuid,
    pub bytes: Vec<u8>,
    pub replace: bool,
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

    pub fn show_block(&self, block_id: Uuid, block_type: Uuid, via: Option<Uuid>) {
        self.shows.borrow_mut().push(ShowRequest {
            block_id,
            block_type,
            via,
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

    #[cfg(target_arch = "wasm32")]
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

    pub fn set_block_content(&self, content_type: Uuid, bytes: Vec<u8>, applied: u64) {
        self.update_content(
            None,
            ContentUpdate::Snapshot(HostContent {
                content_type,
                bytes,
                applied,
            }),
        );
    }

    pub fn set_content_of(&self, block: Uuid, content_type: Uuid, bytes: Vec<u8>, applied: u64) {
        self.update_content(
            Some(block),
            ContentUpdate::Snapshot(HostContent {
                content_type,
                bytes,
                applied,
            }),
        );
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_all_content_operations(&self) -> Vec<(Option<Uuid>, Vec<u8>)> {
        std::mem::take(&mut self.content_operations.borrow_mut())
    }

    pub(crate) fn watch_content(&self, block: Uuid, content_type: Uuid) {
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
        self.seeded.borrow_mut().push(SeededContent {
            block,
            content_type: C::CONTENT_TYPE,
            bytes: content.encode(),
            replace,
        });
        self.waker.wake();
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

    #[cfg(target_arch = "wasm32")]
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

    pub fn view(&self) -> Option<beui::Rect> {
        let origin = self.region.get().origin;
        self.view.get().map(|view| view.rect.translate(origin))
    }

    pub fn view_scale(&self) -> Option<f32> {
        self.view.get().map(|view| view.scale)
    }

    pub fn pan_view(&self, delta: beui::Vec2) {
        self.view_changes.borrow_mut().push(ViewChange::Pan {
            x: delta.x,
            y: delta.y,
        });
    }

    pub fn zoom_view(&self, factor: f32, anchor: Option<beui::Pos2>) {
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

    pub fn pick_block(&self, filter: BlockFilter) -> u64 {
        self.ask(HostRequest::PickBlock(filter))
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_audio_commands(&self) -> Vec<(Uuid, AudioCommand)> {
        std::mem::take(&mut self.audio_commands.borrow_mut())
    }

    pub fn set_audio(&self, status: AudioStatus) {
        *self.audio_status.borrow_mut() = status;
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

    pub fn take_requests(&self) -> Vec<(u64, HostRequest)> {
        std::mem::take(&mut self.requests.borrow_mut())
    }

    pub fn set_reply(&self, request: u64, reply: HostReply) {
        self.replies.borrow_mut().insert(request, reply);
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

    pub fn place_web_view(&self, rect: Option<beui::Rect>) {
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

    pub fn occlude(&self, rect: beui::Rect) {
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
        top_bar: bool,
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

    pub fn occlude_beui(&self, rect: beui::Rect) {
        let ratio = self.beui.get().ratio;
        self.occlude(host_rect(rect, ratio));
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

    pub fn set_view(&self, view: beui::Rect, scale: f32) {
        self.view.set(Some(View { rect: view, scale }));
    }

    pub fn set_beui_view(&self, view: beui::Rect, scale: f32) {
        let ratio = self.beui.get().ratio;
        let origin = self.region.get().origin;
        self.set_view(host_rect(view, ratio).translate(-origin), scale);
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
            position: beui::pos2(drag.position.x / ratio, drag.position.y / ratio),
            block_id: drag.block_id,
            block_type: drag.block_type,
            dropped: drag.dropped,
        }));
    }

    pub fn take_drag_accepted(&self) -> Option<bool> {
        self.drag_accepted.take()
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn begin_region(&self, region: EditorRegion, origin: beui::Vec2) {
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

    pub(crate) fn take_child_view_changes(&self, child: ChildId) -> Vec<ViewChange> {
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

fn child_rect(rect: beui::Rect) -> ChildRect {
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
