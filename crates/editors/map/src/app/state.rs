use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_beui::be_block::Edit;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{Image, Pos2, Rect, Vec2};
use block_editor_beui::block_ui::BlockLabel;
use block_editor_beui::{BlockFilter, ContentProjection, Drag, Editor, FileDrop, Waker};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use map_types::{Map, MapColor, MapContent, MapCoordinate, MapPoint, MapRegion};
use uuid::Uuid;

use crate::geo::MapView;
use crate::raster::{TILE_PIXELS, TileLabel};
use crate::tiles::{TileId, TileWorker};
use image_block_types::{ImageContent, ImageFile};

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

struct MapInner {
    block: ContentProjection<MapContent>,
    worker: RefCell<Option<TileWorker>>,
    tiles: RefCell<HashMap<TileId, TileState>>,
    used: RefCell<HashMap<TileId, u64>>,
    uses: Cell<u64>,
    waker: RefCell<Waker>,
    dragged: Cell<Option<(Uuid, Vec2)>>,
    pending_file_drop: Cell<Option<MapCoordinate>>,
    view_center: Cell<MapCoordinate>,
}

#[derive(Clone, Copy)]
pub(crate) struct MapState {
    editor: Editor,
    preview: bool,
    fit_requested: Signal<bool>,
    pub(crate) points: ReadSignal<Vec<MapPoint>>,
    pub(crate) preview_region: ReadSignal<Option<MapRegion>>,
    pub(crate) displayed_region: ReadSignal<MapRegion>,
    pub(crate) selected: Signal<Option<Uuid>>,
    pub(crate) visible_region: Signal<MapRegion>,
    pub(crate) last_error: Signal<Option<String>>,
    pub(crate) import_error: Signal<Option<String>>,
    pub(crate) labels: Memo<Ready<HashMap<Uuid, BlockLabel>>>,
    pub(crate) revision: Signal<u64>,
    pub(crate) reloads: Signal<u64>,
    pub(crate) anchor: Signal<[f64; 2]>,
    inner: Stored<MapInner>,
}

impl MapState {
    pub(crate) fn new(editor: Editor, preview: bool) -> Self {
        let block = editor.block_content::<MapContent>();
        let points = block.project(|map| map.root().points());
        let preview_region = block.project(|map| map.root().preview_region);
        let displayed_region = block.project(|map| map.root().displayed_region());
        let selected = create_signal(None);
        let visible_region = create_signal(MapRegion::WORLD);
        let last_error = create_signal(None);
        let import_error = create_signal(None);
        let references = editor.watch_blocks(BlockQuery::References(editor.block_id()));
        let labels = create_memo(move || {
            let types = editor.block_types();
            Ok(references
                .get()?
                .into_iter()
                .map(|reference: BlockInfo| (reference.id, reference.label(types.as_ref())))
                .collect())
        });
        let revision = create_signal(0);
        let reloads = create_signal(0);
        let fit_requested = create_signal(true);
        let anchor = create_signal([0.0, 0.0]);
        Self {
            editor,
            preview,
            fit_requested,
            points,
            preview_region,
            displayed_region,
            selected,
            visible_region,
            last_error,
            import_error,
            labels,
            revision,
            reloads,
            anchor,
            inner: store(MapInner {
                block,
                worker: RefCell::new(None),
                tiles: RefCell::new(HashMap::new()),
                used: RefCell::new(HashMap::new()),
                uses: Cell::new(0),
                waker: RefCell::new(Waker::default()),
                dragged: Cell::new(None),
                pending_file_drop: Cell::new(None),
                view_center: Cell::new(MapRegion::WORLD.center()),
            }),
        }
    }

    pub(crate) fn editor(&self) -> Editor {
        self.editor
    }

    pub(crate) fn block_id(&self) -> Uuid {
        self.editor.block_id()
    }

    pub(crate) fn with_tiles<R>(&self, read: impl FnOnce(&HashMap<TileId, TileState>) -> R) -> R {
        self.inner.with(|inner| read(&inner.tiles.borrow()))
    }

    pub(crate) fn select(&self, id: Option<Uuid>) {
        self.selected.set(id);
    }

    pub(crate) fn dismiss_import_error(&self) {
        self.import_error.set(None);
    }

    pub(crate) fn request_fit(&self) {
        self.fit_requested.set(true);
    }

    pub(crate) fn reload_tiles(&self) {
        let inner = self.inner.value();
        if let Some(worker) = inner.worker.borrow_mut().take() {
            worker.forget();
        }
        inner.tiles.borrow_mut().clear();
        inner.used.borrow_mut().clear();
        self.last_error.set(None);
        self.bump();
        self.reloads.update(|reloads| *reloads += 1);
    }

    fn bump(&self) {
        self.revision.update(|revision| *revision += 1);
    }

    pub(crate) fn record(&self, edit: Edit) {
        let inner = self.inner.value();
        inner.block.operate(edit);
    }

    pub(crate) fn remove_point(&self, id: Uuid) {
        if self.selected.get_untracked() == Some(id) {
            self.selected.set(None);
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
        self.selected.set(Some(point_id));
    }

    pub(crate) fn open_picker(self, at: Option<MapCoordinate>, anchor: Option<Rect>) {
        self.editor
            .pick_block(BlockFilter::default(), anchor, move |picked| {
                if let (true, Ok(picked)) = (self.inner.is_alive(), picked) {
                    self.place_picked(picked.id(), at);
                }
            });
    }

    fn place_picked(&self, block_id: Uuid, at: Option<MapCoordinate>) {
        let inner = self.inner.value();
        self.editor
            .blocks()
            .set_parent(block_id, BlockParent::Block(self.block_id()));
        let position = at.unwrap_or_else(|| inner.view_center.get());
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
            self.anchor.set(anchor);
        }
    }

    pub(crate) fn label_of(&self, block: Uuid) -> Ready<BlockLabel> {
        self.labels
            .get()?
            .get(&block)
            .cloned()
            .ok_or(Pending::Loading)
    }

    pub(crate) fn name_of(&self, block: Uuid) -> String {
        match self.label_of(block) {
            Ok(label) => label.name,
            Err(Pending::Loading) => "Loading…".to_owned(),
            Err(Pending::Failed(error)) => error,
        }
    }

    pub(crate) fn press(&self, at: Pos2) {
        let inner = self.inner.value();
        let view = self.view();
        let points = self.points.get_untracked();
        let tip = |point: &MapPoint| self.screen(view.position(point.position));
        let hit = crate::points::point_at(&points, tip, at);
        if hit.is_none() {
            self.editor.pan_drag();
        }
        self.selected.set(hit);
        inner.dragged.set(hit.and_then(|id| {
            let point = points.iter().find(|point| point.id == id)?;
            Some((id, tip(point) - at))
        }));
    }

    pub(crate) fn drag(&self, at: Pos2) {
        let inner = self.inner.value();
        let Some((id, offset)) = inner.dragged.get() else {
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
        let inner = self.inner.value();
        inner.dragged.take();
    }

    pub(crate) fn pan(&self, delta: Vec2) {
        let inner = self.inner.value();
        if inner.dragged.get().is_none() {
            self.editor.pan(delta);
        }
    }

    pub(crate) fn zoom(&self, factor: f32) {
        self.editor.zoom(factor);
    }

    pub(crate) fn watch(self) {
        let inner = self.inner.value();
        let (waker, woken) = self.editor.woken();
        *inner.waker.borrow_mut() = waker;
        create_effect(move || {
            woken.with(|_| ());
            untrack(|| self.collect_tiles());
        });
        let drag = self.editor.drag();
        create_effect(move || {
            let drag = drag.get();
            untrack(|| self.take_drag(drag));
        });
        let files = self.editor.files();
        create_effect(move || {
            let drop = files.get();
            untrack(|| self.take_files(drop));
        });
        if self.preview {
            return;
        }
        let world = self.editor.world();
        let placed = self.editor.placed();
        let canvas = self.editor.canvas();
        create_effect(move || {
            world.with(|_| ());
            placed.with(|_| ());
            canvas.with(|_| ());
            untrack(|| self.settle_anchor());
        });
        let world = self.editor.world();
        let placed = self.editor.placed();
        let canvas = self.editor.canvas();
        create_effect(move || {
            world.with(|_| ());
            placed.with(|_| ());
            canvas.with(|_| ());
            self.anchor.with(|_| ());
            self.preview_region.with(|_| ());
            self.fit_requested.with(|_| ());
            untrack(|| self.settle_view());
        });
    }

    fn settle_view(&self) {
        let inner = self.inner.value();
        let view = self.view();
        let visible = untrack(|| self.visible());
        let region = view.region(visible);
        if self.visible_region.get_untracked() != region {
            self.visible_region.set(region);
        }
        inner.view_center.set(view.coordinate(visible.center()));
        if self.sized()
            && self.host_camera().is_some()
            && self.fit_requested.get_untracked()
            && let Some(preview) = self.preview_region.get_untracked()
        {
            self.fit_requested.set(false);
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
        let inner = self.inner.value();
        let Some(drop) = drop else {
            inner.pending_file_drop.set(None);
            return;
        };
        let view = self.view();
        let at = |position: Pos2| view.coordinate(self.unscreen(position));
        if !drop.dropped {
            inner.pending_file_drop.set(Some(at(drop.position)));
            return;
        }
        self.import_error.set(None);
        let base = inner
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
            self.import(ImageFile::from_file(file.name, file.data), position);
        }
    }

    pub(crate) fn take_paste(&self, pasted: Pasted) {
        let inner = self.inner.value();
        if let Pasted::Image(file) = pasted {
            self.import_error.set(None);
            let position = inner.view_center.get();
            self.import(ImageFile::from_file(file.name, file.data), position);
        }
    }

    fn import(&self, image: ImageContent, position: MapCoordinate) {
        let created = self.editor.create_child(&image);
        self.add_point(created, position);
    }

    pub(crate) fn unwant_tile(&self, id: TileId) {
        let Some(inner) = self.inner.try_value() else {
            return;
        };
        if !matches!(inner.tiles.borrow().get(&id), Some(TileState::Loading)) {
            return;
        }
        let cancelled = inner
            .worker
            .borrow_mut()
            .as_mut()
            .is_some_and(|worker| worker.cancel(id));
        if cancelled {
            inner.tiles.borrow_mut().remove(&id);
        }
    }

    pub(crate) fn use_tiles(&self, shown: &[TileId]) {
        let inner = self.inner.value();
        let uses = inner.uses.get() + 1;
        inner.uses.set(uses);
        let mut used = inner.used.borrow_mut();
        for tile in shown {
            let mut held = Some(*tile);
            while let Some(tile) = held {
                used.insert(tile, uses);
                held = tile.parent();
            }
        }
        let mut tiles = inner.tiles.borrow_mut();
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
        let inner = self.inner.value();
        let mut tiles = inner.tiles.borrow_mut();
        if tiles.contains_key(&id) {
            return;
        }
        tiles.insert(id, TileState::Loading);
        drop(tiles);
        let host = self.editor.host();
        let mut worker = inner.worker.borrow_mut();
        let worker = worker.get_or_insert_with(|| TileWorker::spawn(inner.waker.borrow().clone()));
        worker.request(id);
        worker.dispatch(&host);
    }

    fn collect_tiles(&self) {
        let inner = self.inner.value();
        let host = self.editor.host();
        let results = match inner.worker.borrow_mut().as_mut() {
            Some(worker) => worker.poll(&host),
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
                    self.last_error.set(Some(message));
                    TileState::Failed
                }
            };
            inner.tiles.borrow_mut().insert(result.id, state);
        }
        self.bump();
    }
}
