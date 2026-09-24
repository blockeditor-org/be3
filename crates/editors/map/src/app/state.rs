use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use block::{BlockParent, BlockReference, BlockReferenceList};
use block_client::ReferenceList;
use block_client::blocks::image::Image as ImageBlock;
use block_client::blocks::map::{MapColor, MapCoordinate, MapPoint, MapRegion};
use block_editor_plugin::be_block::ImageContent;
use block_editor_plugin::be_block::{Edit, Map, MapContent};
use block_editor_plugin::beui::reactive::{ReadSignal, WriteSignal, create_signal};
use block_editor_plugin::beui::{Image, Pos2, Rect, Vec2};
use block_editor_plugin::block_ui::{BlockCatalog, BlockLabel};
use block_editor_plugin::{
    BlockFilter, BlockPicker, ContentProjection, Editor, ImagePaster, PastedImage,
};
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
    dependencies: ReferenceList,
    worker: RefCell<Option<TileWorker>>,
    tiles: RefCell<HashMap<TileId, TileState>>,
    picker: RefCell<BlockPicker>,
    paster: RefCell<ImagePaster>,
    pending_points: RefCell<Vec<(Uuid, (Uuid, MapCoordinate))>>,
    pending_position: Cell<Option<MapCoordinate>>,
    paste_asked: Cell<bool>,
    dragged: Cell<Option<(Uuid, Vec2)>>,
    pending_file_drop: Cell<Option<MapCoordinate>>,
    fit_requested: Cell<bool>,
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
        Rc::new(Self {
            dependencies: editor
                .client()
                .watch_references(BlockReferenceList::References(editor.block_id())),
            editor: editor.clone(),
            preview,
            block,
            worker: RefCell::new(None),
            tiles: RefCell::new(HashMap::new()),
            picker: RefCell::new(BlockPicker::default()),
            paster: RefCell::new(ImagePaster::default()),
            pending_points: RefCell::new(Vec::new()),
            pending_position: Cell::new(None),
            paste_asked: Cell::new(false),
            dragged: Cell::new(None),
            pending_file_drop: Cell::new(None),
            fit_requested: Cell::new(true),
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
        self.editor.host().block_types()
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
        self.fit_requested.set(true);
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
        self.pending_points
            .borrow_mut()
            .push((block_id, (point_id, position)));
        self.set_selected.set(Some(point_id));
    }

    pub(crate) fn open_picker(&self, at: Option<MapCoordinate>) {
        self.pending_position.set(at);
        self.picker
            .borrow_mut()
            .open(self.editor.host(), BlockFilter::default());
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
        self.paste_asked.set(true);
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

    pub(crate) fn poll(&self) {
        self.poll_pending_points();
        self.poll_tiles();
        self.poll_picker();
        self.poll_drag();
        self.poll_files();
        self.poll_clipboard();
        self.publish_references();
        self.settle_view();
    }

    fn settle_view(&self) {
        if self.preview {
            return;
        }
        let region = self.content_rect();
        let view = self.view();
        self.set_visible_region.set(view.region(region));
        self.view_center.set(view.coordinate(region.center()));
        if self.fit_requested.get()
            && let Some(preview) = self.preview_region.get_untracked()
        {
            self.fit_requested.set(false);
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
            .map(|reference: BlockReference| {
                (
                    reference.id,
                    BlockLabel::for_reference(types.as_ref(), &reference),
                )
            })
            .collect();
        if self.labels.get_untracked() != labels {
            self.set_labels.set(labels);
        }
    }

    fn poll_pending_points(&self) {
        let landed = std::mem::take(&mut *self.pending_points.borrow_mut());
        for (reference, (point_id, position)) in landed {
            self.record(Map::add(&MapPoint {
                id: point_id,
                block_id: reference,
                position,
                color: MapColor::Default,
            }));
        }
    }

    fn poll_picker(&self) {
        let picked = self.picker.borrow_mut().poll(self.editor.host());
        let Some(Ok(picked)) = picked else {
            return;
        };
        self.editor
            .client()
            .set_block_parent(picked.id, BlockParent::Uuid(self.block_id()));
        let position = self
            .pending_position
            .take()
            .unwrap_or_else(|| self.view_center.get());
        self.add_point(picked.id, position);
    }

    fn poll_drag(&self) {
        let Some(drag) = self.editor.drag().get_untracked() else {
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
            .client()
            .set_block_parent(drag.block_id, BlockParent::Uuid(self.block_id()));
        self.add_point(drag.block_id, position);
    }

    fn poll_files(&self) {
        let Some(drop) = self.editor.host().files() else {
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

    fn poll_clipboard(&self) {
        let asked = self.paste_asked.take();
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
        let created = self.editor.create_with_content::<ImageBlock, _>(&image);
        created.set_parent(BlockParent::Uuid(self.block_id()));
        self.add_point(created.id(), position);
    }

    pub(crate) fn want_tile(&self, id: TileId) {
        let mut tiles = self.tiles.borrow_mut();
        if tiles.contains_key(&id) {
            return;
        }
        tiles.insert(id, TileState::Loading);
        drop(tiles);
        if let Some(worker) = self.worker.borrow_mut().as_mut() {
            worker.request(id);
        }
    }

    fn poll_tiles(&self) {
        let host = self.editor.host().clone();
        let mut worker = self.worker.borrow_mut();
        let worker = worker.get_or_insert_with(|| TileWorker::spawn(host.waker()));
        let results = worker.poll(&host);
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
