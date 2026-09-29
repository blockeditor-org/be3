use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_beui::BlockList;
use block_editor_beui::be_block::ImageContent;
use block_editor_beui::be_block::map::{MapColor, MapCoordinate, MapPoint, MapRegion};
use block_editor_beui::be_block::{Edit, Map, MapContent};
use block_editor_beui::beui::reactive::{
    CanvasView, ReadSignal, WriteSignal, create_effect, create_signal, untrack,
};
use block_editor_beui::beui::{Image, Pos2, Rect, Vec2};
use block_editor_beui::block_ui::{BlockCatalog, BlockLabel};
use block_editor_beui::{
    BlockFilter, ContentProjection, Drag, Editor, FileDrop, ImagePaster, PastedImage, Waker,
};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use uuid::Uuid;

use crate::geo::MapView;
use crate::raster::{TILE_PIXELS, TileLabel};
use crate::tiles::{TileId, TileWorker};

pub(crate) const WORLD_POINTS: f32 = 1024.0;
pub(crate) const MAX_SCALE: f32 = 32768.0;
pub(crate) const MAX_PREVIEW_WORLD: f64 = (WORLD_POINTS * MAX_SCALE) as f64;
pub(crate) const ZOOM_STEP: f32 = 1.25;
const MAX_HELD_TILES: usize = 160;

#[derive(Clone)]
pub(crate) enum TileState {
    Loading,
    Ready {
        image: Image,
        labels: Rc<Vec<TileLabel>>,
    },
    Failed,
}

impl TileState {
    pub(crate) fn image(&self) -> Option<&Image> {
        match self {
            TileState::Ready { image, .. } => Some(image),
            _ => None,
        }
    }
}

pub(crate) struct MapState {
    editor: Editor,
    preview: bool,
    block: Rc<ContentProjection<MapContent>>,
    dependencies: BlockList,
    worker: RefCell<Option<TileWorker>>,
    tiles: RefCell<HashMap<TileId, TileState>>,
    used: RefCell<HashMap<TileId, u64>>,
    uses: Cell<u64>,
    waker: RefCell<Waker>,
    paster: RefCell<ImagePaster>,
    dragged: Cell<Option<(Uuid, Vec2)>>,
    pending_file_drop: Cell<Option<MapCoordinate>>,
    fit_requested: ReadSignal<bool>,
    set_fit_requested: WriteSignal<bool>,
    view_center: Cell<MapCoordinate>,
    pub(crate) points: ReadSignal<Vec<MapPoint>>,
    pub(crate) preview_region: ReadSignal<Option<MapRegion>>,
    pub(crate) displayed_region: ReadSignal<MapRegion>,
    pub(crate) selected: ReadSignal<Option<Uuid>>,
    set_selected: WriteSignal<Option<Uuid>>,
    pub(crate) visible_region: ReadSignal<MapRegion>,
    set_visible_region: WriteSignal<MapRegion>,
    pub(crate) last_error: ReadSignal<Option<String>>,
    set_last_error: WriteSignal<Option<String>>,
    pub(crate) import_error: ReadSignal<Option<String>>,
    set_import_error: WriteSignal<Option<String>>,
    pub(crate) labels: ReadSignal<HashMap<Uuid, BlockLabel>>,
    set_labels: WriteSignal<HashMap<Uuid, BlockLabel>>,
    pub(crate) revision: ReadSignal<u64>,
    set_revision: WriteSignal<u64>,
    pub(crate) reloads: ReadSignal<u64>,
    set_reloads: WriteSignal<u64>,
    pub(crate) anchor: ReadSignal<[f64; 2]>,
    set_anchor: WriteSignal<[f64; 2]>,
}

impl MapState {
    pub(crate) fn new(editor: &Editor, preview: bool) -> Rc<Self> {
        let block = editor.block_content::<MapContent>();
        let points = block.project(|map| map.root().points());
        let preview_region = block.project(|map| map.root().preview_region);
        let displayed_region = block.project(|map| map.root().displayed_region());
        let (selected, set_selected) = create_signal(None);
        let (visible_region, set_visible_region) = create_signal(MapRegion::WORLD);
        let (last_error, set_last_error) = create_signal(None);
        let (import_error, set_import_error) = create_signal(None);
        let (labels, set_labels) = create_signal(HashMap::new());
        let (revision, set_revision) = create_signal(0);
        let (reloads, set_reloads) = create_signal(0);
        let (fit_requested, set_fit_requested) = create_signal(true);
        let (anchor, set_anchor) = create_signal([0.0, 0.0]);
        Rc::new(Self {
            dependencies: editor
                .blocks()
                .watch(BlockQuery::References(editor.block_id())),
            editor: editor.clone(),
            preview,
            block,
            worker: RefCell::new(None),
            tiles: RefCell::new(HashMap::new()),
            used: RefCell::new(HashMap::new()),
            uses: Cell::new(0),
            waker: RefCell::new(Waker::default()),
            paster: RefCell::new(ImagePaster::default()),
            dragged: Cell::new(None),
            pending_file_drop: Cell::new(None),
            fit_requested,
            set_fit_requested,
            view_center: Cell::new(MapRegion::WORLD.center()),
            points,
            preview_region,
            displayed_region,
            selected,
            set_selected,
            visible_region,
            set_visible_region,
            last_error,
            set_last_error,
            import_error,
            set_import_error,
            labels,
            set_labels,
            revision,
            set_revision,
            reloads,
            set_reloads,
            anchor,
            set_anchor,
        })
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.editor
    }

    pub(crate) fn block_id(&self) -> Uuid {
        self.editor.block_id()
    }

    pub(crate) fn types(&self) -> Rc<BlockCatalog> {
        self.editor.block_types()
    }

    pub(crate) fn tiles(&self) -> std::cell::Ref<'_, HashMap<TileId, TileState>> {
        self.tiles.borrow()
    }

    pub(crate) fn select(&self, id: Option<Uuid>) {
        self.set_selected.set(id);
    }

    pub(crate) fn dismiss_import_error(&self) {
        self.set_import_error.set(None);
    }

    pub(crate) fn request_fit(&self) {
        self.set_fit_requested.set(true);
    }

    pub(crate) fn reload_tiles(&self) {
        if let Some(worker) = self.worker.borrow_mut().take() {
            worker.forget(self.editor.host());
        }
        self.tiles.borrow_mut().clear();
        self.used.borrow_mut().clear();
        self.set_last_error.set(None);
        self.bump();
        self.set_reloads.update(|reloads| *reloads += 1);
    }

    fn bump(&self) {
        self.set_revision.update(|revision| *revision += 1);
    }

    pub(crate) fn record(&self, edit: Edit) {
        self.block.operate(edit);
    }

    pub(crate) fn remove_point(&self, id: Uuid) {
        if self.selected.get_untracked() == Some(id) {
            self.set_selected.set(None);
        }
        self.record(Map::remove(&[id]));
    }

    pub(crate) fn add_point(&self, block_id: Uuid, position: MapCoordinate) {
        let point_id = Uuid::new_v4();
        self.record(Map::add(&MapPoint {
            id: point_id,
            block_id,
            position,
            color: MapColor::Default,
        }));
        self.set_selected.set(Some(point_id));
    }

    pub(crate) fn open_picker(self: &Rc<Self>, at: Option<MapCoordinate>) {
        let state = Rc::downgrade(self);
        self.editor
            .pick_block(BlockFilter::default(), move |picked| {
                if let (Some(state), Ok(picked)) = (state.upgrade(), picked) {
                    state.place_picked(picked.id, at);
                }
            });
    }

    fn place_picked(&self, block_id: Uuid, at: Option<MapCoordinate>) {
        self.editor
            .blocks()
            .set_parent(block_id, BlockParent::Block(self.block_id()));
        let position = at.unwrap_or_else(|| self.view_center.get());
        self.add_point(block_id, position);
    }

    pub(crate) fn centre_on(&self, position: MapCoordinate) {
        let shown = self.screen(self.view().position(position));
        self.editor.pan(self.screen_rect().center() - shown);
    }

    pub(crate) fn content_rect(&self) -> Rect {
        let size = self
            .editor
            .world()
            .get_untracked()
            .unwrap_or_else(|| self.editor.content_rect().size())
            .max(Vec2::new(1.0, 1.0));
        Rect::from_min_size(Pos2::ZERO, size)
    }

    pub(crate) fn sized(&self) -> bool {
        self.editor.world().get_untracked().is_some()
            || self.editor.placed().get_untracked().is_positive()
    }

    fn host_camera(&self) -> Option<CanvasView> {
        match self.preview {
            true => None,
            false => self.editor.canvas().get_untracked(),
        }
    }

    pub(crate) fn camera(&self) -> Option<CanvasView> {
        let _ = self.editor.canvas().get();
        let _ = self.anchor.get();
        self.anchored_camera()
    }

    fn anchored_camera(&self) -> Option<CanvasView> {
        let host = self.host_camera()?;
        let anchor = self.anchor.get_untracked();
        let scale = f64::from(host.scale);
        Some(CanvasView::new(
            Pos2::new(
                (f64::from(host.origin.x) + anchor[0] * scale) as f32,
                (f64::from(host.origin.y) + anchor[1] * scale) as f32,
            ),
            host.scale,
        ))
    }

    fn screen_rect(&self) -> Rect {
        let placed = self.editor.placed().get_untracked();
        match self.host_camera().is_some() && placed.is_positive() {
            true => placed,
            false => self.content_rect(),
        }
    }

    pub(crate) fn screen(&self, position: Pos2) -> Pos2 {
        self.anchored_camera()
            .map_or(position, |camera| camera.to_screen(position))
    }

    fn unscreen(&self, position: Pos2) -> Pos2 {
        self.anchored_camera()
            .map_or(position, |camera| camera.to_canvas(position))
    }

    pub(crate) fn visible(&self) -> Rect {
        let _ = self.editor.canvas().get();
        let _ = self.editor.placed().get();
        let _ = self.anchor.get();
        let screen = self.screen_rect();
        Rect::from_min_max(self.unscreen(screen.min), self.unscreen(screen.max))
    }

    fn world_view(&self) -> MapView {
        match self.preview {
            true => MapView::covering(
                self.displayed_region.get_untracked(),
                self.content_rect(),
                MAX_PREVIEW_WORLD,
            ),
            false => {
                let content = self.content_rect();
                let side = content.width().min(content.height());
                let centre = content.center();
                MapView::from_world_rect(Rect::from_min_size(
                    Pos2::new(centre.x - side / 2.0, centre.y - side / 2.0),
                    Vec2::splat(side),
                ))
            }
        }
    }

    pub(crate) fn view(&self) -> MapView {
        self.world_view().anchored(self.anchor.get_untracked())
    }

    fn settle_anchor(&self) {
        let Some(host) = self.host_camera() else {
            return;
        };
        let world = self.world_view();
        let centre = host.to_canvas(self.screen_rect().center());
        let zoom = super::tiles::tile_zoom(world, host.scale);
        let anchor = world.tile_anchor(zoom, centre);
        if self.anchor.get_untracked() != anchor {
            self.set_anchor.set(anchor);
        }
    }

    pub(crate) fn label_of(&self, block: Uuid) -> Option<BlockLabel> {
        self.labels.get().get(&block).cloned()
    }

    pub(crate) fn ask_to_paste(&self) {
        self.take_paste(true);
    }

    pub(crate) fn press(&self, at: Pos2) {
        let view = self.view();
        let points = self.points.get_untracked();
        let tip = |point: &MapPoint| self.screen(view.position(point.position));
        let hit = crate::points::point_at(&points, tip, at);
        self.set_selected.set(hit);
        self.dragged.set(hit.and_then(|id| {
            let point = points.iter().find(|point| point.id == id)?;
            Some((id, tip(point) - at))
        }));
    }

    pub(crate) fn drag(&self, at: Pos2) {
        let Some((id, offset)) = self.dragged.get() else {
            return;
        };
        let view = self.view();
        let points = self.points.get_untracked();
        let Some(mut point) = points.iter().copied().find(|point| point.id == id) else {
            return;
        };
        let position = view.coordinate(self.unscreen(at + offset));
        if position == point.position {
            return;
        }
        point.position = position;
        self.record(Map::update(&[point]));
    }

    pub(crate) fn release(&self) {
        self.dragged.take();
    }

    pub(crate) fn pan(&self, delta: Vec2) {
        if self.dragged.get().is_none() {
            self.editor.pan(delta);
        }
    }

    pub(crate) fn zoom(&self, factor: f32) {
        self.editor.zoom(factor);
    }

    pub(crate) fn watch(self: &Rc<Self>) {
        let (waker, woken) = self.editor.woken();
        *self.waker.borrow_mut() = waker;
        let state = Rc::clone(self);
        create_effect(move || {
            woken.with(|_| ());
            untrack(|| state.collect_tiles());
        });
        let state = Rc::clone(self);
        self.editor.on_reply(move || {
            state.collect_tiles();
            state.take_paste(false);
        });
        let state = Rc::clone(self);
        let drag = self.editor.drag();
        create_effect(move || {
            let drag = drag.get();
            untrack(|| state.take_drag(drag));
        });
        let state = Rc::clone(self);
        let files = self.editor.files();
        create_effect(move || {
            let drop = files.get();
            untrack(|| state.take_files(drop));
        });
        let state = Rc::clone(self);
        create_effect(move || state.publish_references());
        if self.preview {
            return;
        }
        let state = Rc::clone(self);
        let world = self.editor.world();
        let placed = self.editor.placed();
        let canvas = self.editor.canvas();
        create_effect(move || {
            world.with(|_| ());
            placed.with(|_| ());
            canvas.with(|_| ());
            untrack(|| state.settle_anchor());
        });
        let state = Rc::clone(self);
        let world = self.editor.world();
        let placed = self.editor.placed();
        let canvas = self.editor.canvas();
        create_effect(move || {
            world.with(|_| ());
            placed.with(|_| ());
            canvas.with(|_| ());
            state.anchor.with(|_| ());
            state.preview_region.with(|_| ());
            state.fit_requested.with(|_| ());
            untrack(|| state.settle_view());
        });
    }

    fn settle_view(&self) {
        let view = self.view();
        let visible = untrack(|| self.visible());
        let region = view.region(visible);
        if self.visible_region.get_untracked() != region {
            self.set_visible_region.set(region);
        }
        self.view_center.set(view.coordinate(visible.center()));
        if self.sized()
            && self.host_camera().is_some()
            && self.fit_requested.get_untracked()
            && let Some(preview) = self.preview_region.get_untracked()
        {
            self.set_fit_requested.set(false);
            self.fit_preview_region(view, preview);
        }
    }

    fn fit_preview_region(&self, view: MapView, region: MapRegion) {
        let rect = view.region_rect(region);
        let shown = Rect::from_min_max(self.screen(rect.min), self.screen(rect.max));
        let clip = self.screen_rect();
        let available = (clip.size() - Vec2::splat(24.0)).max(Vec2::new(1.0, 1.0));
        let factor =
            (available.x / shown.width().max(0.01)).min(available.y / shown.height().max(0.01));
        self.editor.zoom(factor);
        self.editor.pan((clip.center() - shown.center()) * factor);
    }

    fn publish_references(&self) {
        let types = self.types();
        let labels: HashMap<Uuid, BlockLabel> = self
            .dependencies
            .read()
            .into_iter()
            .map(|reference: BlockInfo| (reference.id, reference.label(types.as_ref())))
            .collect();
        if self.labels.get_untracked() != labels {
            self.set_labels.set(labels);
        }
    }

    fn take_drag(&self, drag: Option<Drag>) {
        let Some(drag) = drag else {
            return;
        };
        if drag.block_id == self.block_id() {
            return;
        }
        self.editor.accept_drag(true);
        if !drag.dropped {
            return;
        }
        let position = self.view().coordinate(self.unscreen(drag.position));
        self.editor
            .blocks()
            .set_parent(drag.block_id, BlockParent::Block(self.block_id()));
        self.add_point(drag.block_id, position);
    }

    fn take_files(&self, drop: Option<FileDrop>) {
        let Some(drop) = drop else {
            self.pending_file_drop.set(None);
            return;
        };
        let view = self.view();
        let at = |position: Pos2| view.coordinate(self.unscreen(position));
        if !drop.dropped {
            self.pending_file_drop.set(Some(at(drop.position)));
            return;
        }
        self.set_import_error.set(None);
        let base = self
            .pending_file_drop
            .take()
            .unwrap_or_else(|| at(drop.position));
        let visible = self.visible_region.get_untracked();
        let step = (visible.east - visible.west) * 0.03;
        for (index, file) in drop.files.into_iter().enumerate() {
            let position = MapCoordinate::new(
                base.longitude + step * index as f64,
                base.latitude - step * index as f64,
            );
            self.import(ImageContent::from_file(file.name, file.data), position);
        }
    }

    fn take_paste(&self, asked: bool) {
        let pasted = self.paster.borrow_mut().paste(self.editor.host(), asked);
        let Some(pasted) = pasted else {
            return;
        };
        match pasted {
            PastedImage::Image { name, data } => {
                self.set_import_error.set(None);
                let position = self.view_center.get();
                self.import(ImageContent::from_file(name, data), position);
            }
            PastedImage::Failed(error) => self.set_import_error.set(Some(error)),
            PastedImage::Empty => {}
        }
    }

    fn import(&self, image: ImageContent, position: MapCoordinate) {
        let created = self.editor.create_child(&image);
        self.add_point(created, position);
    }

    pub(crate) fn unwant_tile(&self, id: TileId) {
        if !matches!(self.tiles.borrow().get(&id), Some(TileState::Loading)) {
            return;
        }
        let cancelled = self
            .worker
            .borrow_mut()
            .as_mut()
            .is_some_and(|worker| worker.cancel(id));
        if cancelled {
            self.tiles.borrow_mut().remove(&id);
        }
    }

    pub(crate) fn use_tiles(&self, shown: &[TileId]) {
        let uses = self.uses.get() + 1;
        self.uses.set(uses);
        let mut used = self.used.borrow_mut();
        for tile in shown {
            let mut held = Some(*tile);
            while let Some(tile) = held {
                used.insert(tile, uses);
                held = tile.parent();
            }
        }
        let mut tiles = self.tiles.borrow_mut();
        if tiles.len() <= MAX_HELD_TILES {
            return;
        }
        let mut idle: Vec<(u64, TileId)> = tiles
            .iter()
            .filter(|(_, state)| !matches!(state, TileState::Loading))
            .map(|(id, _)| (used.get(id).copied().unwrap_or(0), *id))
            .filter(|(last, _)| *last != uses)
            .collect();
        idle.sort_unstable_by_key(|(last, id)| (*last, id.zoom, id.x, id.y));
        let excess = tiles.len() - MAX_HELD_TILES;
        for (_, id) in idle.into_iter().take(excess) {
            tiles.remove(&id);
            used.remove(&id);
        }
    }

    pub(crate) fn want_tile(&self, id: TileId) {
        let mut tiles = self.tiles.borrow_mut();
        if tiles.contains_key(&id) {
            return;
        }
        tiles.insert(id, TileState::Loading);
        drop(tiles);
        let host = self.editor.host();
        let mut worker = self.worker.borrow_mut();
        let worker = worker.get_or_insert_with(|| TileWorker::spawn(self.waker.borrow().clone()));
        worker.request(id);
        worker.dispatch(host);
    }

    fn collect_tiles(&self) {
        let host = self.editor.host();
        let results = match self.worker.borrow_mut().as_mut() {
            Some(worker) => worker.poll(host),
            None => return,
        };
        if results.is_empty() {
            return;
        }
        for result in results {
            let state = match result.result {
                Ok(raster) => TileState::Ready {
                    image: Image::from_rgba(TILE_PIXELS as u32, TILE_PIXELS as u32, raster.pixels),
                    labels: Rc::new(raster.labels),
                },
                Err(message) => {
                    self.set_last_error.set(Some(message));
                    TileState::Failed
                }
            };
            self.tiles.borrow_mut().insert(result.id, state);
        }
        self.bump();
    }
}
