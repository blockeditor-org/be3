use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_plugin::BlockList;
use block_editor_plugin::be_block::ImageContent;
use block_editor_plugin::be_block::map::{MapColor, MapCoordinate, MapPoint, MapRegion};
use block_editor_plugin::be_block::{Edit, Map, MapContent};
use block_editor_plugin::beui::reactive::{
    ReadSignal, WriteSignal, create_effect, create_signal, untrack,
};
use block_editor_plugin::beui::{Image, Pos2, Rect, Vec2};
use block_editor_plugin::block_ui::{BlockCatalog, BlockLabel};
use block_editor_plugin::{
    BlockFilter, ContentProjection, Drag, Editor, FileDrop, ImagePaster, PastedImage, Waker,
};
use block_editor_plugin::{BlockInfo, BlockParent, BlockQuery};
use uuid::Uuid;

use crate::geo::MapView;
use crate::raster::{TILE_PIXELS, TileLabel};
use crate::tiles::{TileId, TileWorker};

pub(crate) const WORLD_POINTS: f32 = 1024.0;
pub(crate) const MAX_PREVIEW_WORLD: f64 = (WORLD_POINTS * 4096.0) as f64;
pub(crate) const ZOOM_STEP: f32 = 1.25;

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
        let (fit_requested, set_fit_requested) = create_signal(true);
        Rc::new(Self {
            dependencies: editor
                .blocks()
                .watch(BlockQuery::References(editor.block_id())),
            editor: editor.clone(),
            preview,
            block,
            worker: RefCell::new(None),
            tiles: RefCell::new(HashMap::new()),
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
        self.worker.borrow_mut().take();
        self.tiles.borrow_mut().clear();
        self.set_last_error.set(None);
        self.bump();
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
        let view = self.view();
        let region = self.content_rect();
        self.editor.pan(region.center() - view.position(position));
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

    pub(crate) fn world_rect(&self) -> Rect {
        if self.preview {
            return self.view().world_rect();
        }
        let content = self.content_rect();
        let side = content.width().min(content.height());
        let centre = content.center();
        Rect::from_min_size(
            Pos2::new(centre.x - side / 2.0, centre.y - side / 2.0),
            Vec2::splat(side),
        )
    }

    pub(crate) fn view(&self) -> MapView {
        match self.preview {
            true => MapView::covering(
                self.displayed_region.get_untracked(),
                self.content_rect(),
                MAX_PREVIEW_WORLD,
            ),
            false => MapView::from_world_rect(self.world_rect()),
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
        let hit = crate::points::point_at(&points, view, at);
        self.set_selected.set(hit);
        self.dragged.set(hit.and_then(|id| {
            let point = points.iter().find(|point| point.id == id)?;
            Some((id, view.position(point.position) - at))
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
        let position = view.coordinate(at + offset);
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
        create_effect(move || {
            world.with(|_| ());
            placed.with(|_| ());
            state.preview_region.with(|_| ());
            state.fit_requested.with(|_| ());
            untrack(|| state.settle_view());
        });
    }

    fn settle_view(&self) {
        let region = self.content_rect();
        let view = self.view();
        self.set_visible_region.set(view.region(region));
        self.view_center.set(view.coordinate(region.center()));
        let sized = self.editor.world().get_untracked().is_some()
            || self.editor.placed().get_untracked().is_positive();
        if sized
            && self.fit_requested.get_untracked()
            && let Some(preview) = self.preview_region.get_untracked()
        {
            self.set_fit_requested.set(false);
            self.fit_preview_region(view, region, preview);
        }
    }

    fn fit_preview_region(&self, view: MapView, clip: Rect, region: MapRegion) {
        let rect = view.region_rect(region);
        let available = (clip.size() - Vec2::splat(24.0)).max(Vec2::new(1.0, 1.0));
        let factor =
            (available.x / rect.width().max(0.01)).min(available.y / rect.height().max(0.01));
        self.editor.zoom(factor);
        self.editor.pan(clip.center() - rect.center());
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
        let position = self.view().coordinate(drag.position);
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
        let at = |position: Pos2| view.coordinate(position);
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
