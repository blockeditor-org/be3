use block_editor_plugin::be_block::{BlockContent, DatabaseSchemaContent};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use crate::presence::CanvasCursor;
use block_editor_plugin::BlockList;
use block_editor_plugin::ContentProjection;
use block_editor_plugin::be_block::ImageContent;
use block_editor_plugin::be_block::canvas::Canvas;
use block_editor_plugin::be_block::canvas::{
    CanvasColor, CanvasComponent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle,
    CanvasLayerMove, CanvasPoint, CanvasPreviewRegion, CanvasTextStyle, CanvasTransform,
    InfiniteCanvasOperation,
};
use block_editor_plugin::be_block::database::DatabaseValue;
use block_editor_plugin::be_block::presence::{PresenceColor, pick_free_color};
use block_editor_plugin::be_block::{CanvasContent, ObjectId};
use block_editor_plugin::beui::reactive::{
    CanvasView, ReadSignal, WriteSignal, create_effect, create_signal, untrack,
};
use block_editor_plugin::beui::{Pos2, Rect, Thumbhash, Vec2};
use block_editor_plugin::block_ui::{BlockCatalog, BlockLabel};
use block_editor_plugin::{
    BlockFilter, ChildState, Drag, Editor, FileDrop, FilePicker, ImagePaster, InteractionMode,
    PastedImage, ResizeMode,
};
use block_editor_plugin::{BlockParent, BlockQuery};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::*;
use crate::images::{image_filter, imported_image};

pub(crate) const ZOOM_STEP: f32 = 1.25;
const CANVAS_CLIPBOARD_PREFIX: &str = "be3-infinite-canvas:";

#[derive(Deserialize, Serialize)]
struct CanvasClipboardPayload {
    entities: Vec<CanvasEntity>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CommonValue<T> {
    None,
    Mixed,
    Uniform(T),
}

impl<T: Copy> CommonValue<T> {
    pub(crate) fn or(self, fallback: T) -> T {
        match self {
            CommonValue::Uniform(value) => value,
            CommonValue::Mixed | CommonValue::None => fallback,
        }
    }

    pub(crate) fn mixed(self) -> bool {
        matches!(self, CommonValue::Mixed)
    }

    pub(crate) fn absent(self) -> bool {
        matches!(self, CommonValue::None)
    }
}

pub(crate) fn common_value<T: Copy + PartialEq>(
    values: impl IntoIterator<Item = T>,
) -> CommonValue<T> {
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return CommonValue::None;
    };
    match values.all(|value| value == first) {
        true => CommonValue::Uniform(first),
        false => CommonValue::Mixed,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Select,
    Line,
    Rectangle,
    Text,
    Pen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alignment {
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CanvasCommand {
    SelectAll,
    InvertSelection,
    Duplicate,
    Delete,
    Lock,
    Unlock,
    Group,
    Ungroup,
    Reorder(CanvasLayerMove),
    Copy,
    Cut,
    Paste,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Gesture {
    Create {
        tool: Tool,
        start: CanvasPoint,
        current: CanvasPoint,
        pointer: CanvasPoint,
        from_center: bool,
    },
    Pen {
        points: Vec<CanvasPoint>,
    },
    SelectBox {
        start: CanvasPoint,
        current: CanvasPoint,
        pointer: CanvasPoint,
        from_center: bool,
        additive: bool,
    },
    Move {
        start: CanvasPoint,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        duplicate: bool,
    },
    Resize {
        handle: ResizeHandle,
        frame: SelectionFrame,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        default_preserve_aspect_ratio: bool,
        force_preserve_aspect_ratio: bool,
        preserve_aspect_ratio: bool,
        scale_text: bool,
        scale_editors: bool,
    },
    Rotate {
        frame: SelectionFrame,
        start_angle: f32,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        snap_angle: bool,
    },
}

pub(crate) fn preview_entities(gesture: &Gesture) -> Vec<CanvasEntity> {
    match gesture {
        Gesture::Move {
            start,
            current,
            originals,
            ..
        } => {
            let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
            originals
                .iter()
                .cloned()
                .map(|mut entity| {
                    entity.transform.center.x += delta.x;
                    entity.transform.center.y += delta.y;
                    entity
                })
                .collect()
        }
        Gesture::Resize {
            handle,
            frame,
            current,
            originals,
            preserve_aspect_ratio,
            scale_text,
            scale_editors,
            ..
        } => resize_entities(
            *handle,
            *frame,
            *current,
            originals,
            *preserve_aspect_ratio,
            *scale_text,
            *scale_editors,
        ),
        Gesture::Rotate {
            frame,
            start_angle,
            current,
            originals,
            snap_angle,
        } => rotate_entities(*frame, *start_angle, *current, originals, *snap_angle),
        Gesture::Create { .. } | Gesture::Pen { .. } | Gesture::SelectBox { .. } => Vec::new(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RemoteCursor {
    pub(crate) pointer: Option<CanvasPoint>,
    pub(crate) color: PresenceColor,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RemoteSelection {
    pub(crate) frame: SelectionFrame,
    pub(crate) color: PresenceColor,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Presence {
    pub(crate) cursors: Vec<RemoteCursor>,
    pub(crate) selections: Vec<RemoteSelection>,
}

pub(crate) struct CanvasState {
    editor: Editor,
    preview: bool,
    content: Rc<ContentProjection<CanvasContent>>,
    dependencies: BlockList,
    image_picker: RefCell<FilePicker>,
    paster: RefCell<ImagePaster>,
    pending_image_center: Cell<Option<CanvasPoint>>,
    pending_file_drop: Cell<Option<CanvasPoint>>,
    context_position: Cell<Option<CanvasPoint>>,
    grouped_edit: Cell<bool>,
    pointer_down: Cell<bool>,
    pub(crate) pointer: ReadSignal<Option<CanvasPoint>>,
    set_pointer: WriteSignal<Option<CanvasPoint>>,
    last_foreground: Cell<CanvasColor>,
    last_fill: Cell<Option<CanvasColor>>,
    children: RefCell<HashMap<Uuid, ChildState>>,
    pub(crate) entities: ReadSignal<Vec<CanvasEntity>>,
    pub(crate) preview_region: ReadSignal<Option<CanvasPreviewRegion>>,
    pub(crate) tool: ReadSignal<Tool>,
    set_tool: WriteSignal<Tool>,
    pub(crate) selection: ReadSignal<HashSet<Uuid>>,
    set_selection: WriteSignal<HashSet<Uuid>>,
    pub(crate) gesture: ReadSignal<Option<Gesture>>,
    set_gesture: WriteSignal<Option<Gesture>>,
    typed: ReadSignal<Option<CanvasEntity>>,
    set_typed: WriteSignal<Option<CanvasEntity>>,
    pub(crate) focused_editor: ReadSignal<Option<Uuid>>,
    set_focused_editor: WriteSignal<Option<Uuid>>,
    confirmed_editor: Cell<Option<Uuid>>,
    pub(crate) editing_text: ReadSignal<Option<Uuid>>,
    set_editing_text: WriteSignal<Option<Uuid>>,
    pub(crate) import_error: ReadSignal<Option<String>>,
    set_import_error: WriteSignal<Option<String>>,
    pub(crate) labels: ReadSignal<HashMap<Uuid, BlockLabel>>,
    set_labels: WriteSignal<HashMap<Uuid, BlockLabel>>,
    pub(crate) types: ReadSignal<HashMap<Uuid, Uuid>>,
    set_types: WriteSignal<HashMap<Uuid, Uuid>>,
    thumbhashes: ReadSignal<HashMap<Uuid, Thumbhash>>,
    set_thumbhashes: WriteSignal<HashMap<Uuid, Thumbhash>>,
    pub(crate) child_states: ReadSignal<HashMap<Uuid, ChildState>>,
    set_child_states: WriteSignal<HashMap<Uuid, ChildState>>,
    pub(crate) presence: ReadSignal<Presence>,
    set_presence: WriteSignal<Presence>,
    peers: ReadSignal<Vec<(u64, CanvasCursor)>>,
    shown: RefCell<Option<CanvasCursor>>,
    color: Cell<Option<PresenceColor>>,
    pub(crate) stage: Cell<Rect>,
}

impl CanvasState {
    pub(crate) fn new(editor: &Editor, preview: bool) -> Rc<Self> {
        let content = editor.block_content::<CanvasContent>();
        let entities = content.project(|canvas| canvas.root().entities());
        let preview_region =
            content.project(|canvas| canvas.field(ObjectId::ROOT, Canvas::PREVIEW_REGION));
        let (tool, set_tool) = create_signal(Tool::Select);
        let (selection, set_selection) = create_signal(HashSet::new());
        let (gesture, set_gesture) = create_signal(None);
        let (typed, set_typed) = create_signal(None);
        let (focused_editor, set_focused_editor) = create_signal(None);
        let (editing_text, set_editing_text) = create_signal(None);
        let (import_error, set_import_error) = create_signal(None);
        let (labels, set_labels) = create_signal(HashMap::new());
        let (types, set_types) = create_signal(HashMap::new());
        let (thumbhashes, set_thumbhashes) = create_signal(HashMap::new());
        let (child_states, set_child_states) = create_signal(HashMap::new());
        let (presence, set_presence) = create_signal(Presence::default());
        let (pointer, set_pointer) = create_signal(None);
        Rc::new(Self {
            dependencies: editor
                .blocks()
                .watch(BlockQuery::References(editor.block_id())),
            editor: editor.clone(),
            preview,
            content,
            image_picker: RefCell::new(FilePicker::default()),
            paster: RefCell::new(ImagePaster::default()),
            pending_image_center: Cell::new(None),
            pending_file_drop: Cell::new(None),
            context_position: Cell::new(None),
            grouped_edit: Cell::new(false),
            pointer_down: Cell::new(false),
            pointer,
            set_pointer,
            last_foreground: Cell::new(CanvasEntityStyle::default().foreground),
            last_fill: Cell::new(CanvasEntityStyle::default().fill),
            children: RefCell::new(HashMap::new()),
            entities,
            preview_region,
            tool,
            set_tool,
            selection,
            set_selection,
            gesture,
            set_gesture,
            typed,
            set_typed,
            focused_editor,
            set_focused_editor,
            confirmed_editor: Cell::new(None),
            editing_text,
            set_editing_text,
            import_error,
            set_import_error,
            labels,
            set_labels,
            types,
            set_types,
            thumbhashes,
            set_thumbhashes,
            child_states,
            set_child_states,
            presence,
            set_presence,
            peers: editor.peers::<CanvasCursor>(),
            shown: RefCell::new(None),
            color: Cell::new(None),
            stage: Cell::new(Rect::ZERO),
        })
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.editor
    }

    pub(crate) fn previewing(&self) -> bool {
        self.preview
    }

    pub(crate) fn block_id(&self) -> Uuid {
        self.editor.block_id()
    }

    pub(crate) fn types(&self) -> Rc<BlockCatalog> {
        self.editor.block_types()
    }

    pub(crate) fn camera(&self) -> CanvasView {
        camera_view(
            self.editor.canvas().get_untracked(),
            self.editor.world().get_untracked(),
            self.stage.get(),
            self.centre(),
        )
    }

    pub(crate) fn centre(&self) -> CanvasPoint {
        match self.preview {
            true => {
                self.preview_region
                    .get()
                    .unwrap_or_else(|| preview_region_for_entities(&self.entities.get()))
                    .center
            }
            false => CanvasPoint::default(),
        }
    }

    pub(crate) fn scale(&self) -> f32 {
        self.camera().scale.max(f32::EPSILON)
    }

    pub(crate) fn world_at(&self, at: Pos2) -> CanvasPoint {
        let canvas = self.camera().to_canvas(at);
        CanvasPoint::new(canvas.x, canvas.y)
    }

    pub(crate) fn visible_world(&self) -> Rect {
        self.camera().rect_to_canvas(self.stage.get())
    }

    pub(crate) fn viewport_center(&self) -> CanvasPoint {
        let center = self.visible_world().center();
        CanvasPoint::new(center.x, center.y)
    }

    pub(crate) fn displayed(&self) -> Vec<CanvasEntity> {
        let mut displayed = displayed_entities(&self.entities.get(), &self.gesture.get());
        if let Some(typed) = self.typed.get()
            && let Some(entity) = displayed.iter_mut().find(|entity| entity.id == typed.id)
        {
            *entity = typed;
        }
        displayed
    }

    pub(crate) fn preview_typed(&self, entity: Option<CanvasEntity>) {
        if self.typed.get_untracked() != entity {
            self.set_typed.set(entity);
        }
    }

    pub(crate) fn label_of(&self, reference: Uuid) -> Option<BlockLabel> {
        self.labels.get().get(&reference).cloned()
    }

    pub(crate) fn thumbhash_of(&self, reference: Uuid) -> Option<Thumbhash> {
        self.thumbhashes.get().get(&reference).cloned()
    }

    pub(crate) fn block_type_of(&self, id: Uuid) -> Option<Uuid> {
        self.types
            .get()
            .get(&id)
            .copied()
            .or_else(|| self.editor.blocks().info(id).map(|info| info.block_type))
    }

    pub(crate) fn child_state(&self, entity: Uuid) -> ChildState {
        self.child_states
            .get()
            .get(&entity)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn report_child(&self, entity: Uuid, state: ChildState) {
        self.children.borrow_mut().insert(entity, state);
        let held = self.children.borrow().clone();
        self.set_child_states.set(held);
    }

    fn peek_child(&self, entity: Uuid) -> ChildState {
        self.children
            .borrow()
            .get(&entity)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn interaction(&self, entity: Uuid) -> Option<InteractionMode> {
        self.peek_child(entity).interaction
    }

    pub(crate) fn resize_mode(&self, entity: &CanvasEntity) -> ResizeMode {
        match entity.kind {
            CanvasEntityKind::DirectEditor { .. } => self.peek_child(entity.id).resize,
            _ => ResizeMode::Both,
        }
    }

    pub(crate) fn shows_preview(&self, entity: Uuid) -> bool {
        let focused = self.focused_editor.get_untracked() == Some(entity);
        !focused && self.interaction(entity) == Some(InteractionMode::Preview)
    }
}

pub(crate) fn camera_view(
    view: Option<CanvasView>,
    world: Option<Vec2>,
    stage: Rect,
    centre: CanvasPoint,
) -> CanvasView {
    let Some(view) = view else {
        let scale = 1.0;
        return CanvasView::new(
            Pos2::new(
                stage.center().x - centre.x * scale,
                stage.center().y - centre.y * scale,
            ),
            scale,
        );
    };
    let scale = view.scale.max(f32::EPSILON);
    let world = world.unwrap_or_else(|| Vec2::new(stage.width() / scale, stage.height() / scale));
    CanvasView::new(
        Pos2::new(
            view.origin.x + (world.x / 2.0 - centre.x) * scale,
            view.origin.y + (world.y / 2.0 - centre.y) * scale,
        ),
        scale,
    )
}

pub(crate) fn displayed_entities(
    entities: &[CanvasEntity],
    gesture: &Option<Gesture>,
) -> Vec<CanvasEntity> {
    let Some(gesture) = gesture else {
        return entities.to_vec();
    };
    let preview: HashMap<Uuid, CanvasEntity> = preview_entities(gesture)
        .into_iter()
        .map(|entity| (entity.id, entity))
        .collect();
    entities
        .iter()
        .map(|entity| preview.get(&entity.id).unwrap_or(entity).clone())
        .chain(
            preview
                .values()
                .filter(|entity| !entities.iter().any(|stored| stored.id == entity.id))
                .cloned(),
        )
        .collect()
}

impl CanvasState {
    fn operate(&self, operation: &InfiniteCanvasOperation) {
        if let Some(edit) = self
            .content
            .read(|canvas| canvas.root().edit_for(operation))
            && !edit.0.is_empty()
        {
            self.content.operate(edit);
        }
    }

    pub(crate) fn record(&self, operation: InfiniteCanvasOperation) {
        self.grouped_edit.set(false);
        self.operate(&operation);
    }

    pub(crate) fn record_update(
        &self,
        before: Vec<CanvasEntity>,
        after: Vec<CanvasEntity>,
        group: bool,
    ) {
        if before == after || after.is_empty() {
            return;
        }
        let operation = InfiniteCanvasOperation::Update { entities: after };
        match group {
            true => {
                self.grouped_edit.set(true);
                self.operate(&operation);
            }
            false => self.record(operation),
        }
    }

    pub(crate) fn finish_grouped_edit(&self) {
        self.grouped_edit.set(false);
    }

    pub(crate) fn default_style(&self) -> CanvasEntityStyle {
        CanvasEntityStyle {
            foreground: self.last_foreground.get(),
            fill: self.last_fill.get(),
            ..CanvasEntityStyle::default()
        }
    }

    pub(crate) fn remember_foreground(&self, color: CanvasColor) {
        self.last_foreground.set(color);
    }

    pub(crate) fn remember_fill(&self, fill: Option<CanvasColor>) {
        self.last_fill.set(fill);
    }

    pub(crate) fn set_tool(&self, tool: Tool) {
        self.set_tool.set(tool);
        self.set_gesture.set(None);
    }

    pub(crate) fn begin_gesture(&self, gesture: Option<Gesture>) {
        self.set_gesture.set(gesture);
    }

    pub(crate) fn update_gesture(&self, update: impl FnOnce(&mut Gesture)) {
        let mut held = self.gesture.get_untracked();
        if let Some(gesture) = held.as_mut() {
            update(gesture);
            self.set_gesture.set(held);
        }
    }

    pub(crate) fn focus_editor(&self, entity: Option<Uuid>) {
        if self.focused_editor.get_untracked() != entity {
            self.set_focused_editor.set(entity);
        }
        if entity.is_none() {
            self.confirmed_editor.set(None);
        }
    }

    pub(crate) fn edit_text(&self, entity: Option<Uuid>) {
        if self.editing_text.get_untracked() != entity {
            self.set_editing_text.set(entity);
        }
    }

    pub(crate) fn fit_text(&self, mut entity: CanvasEntity) -> CanvasEntity {
        let CanvasEntityKind::Text { text_style, .. } = &entity.kind else {
            return entity;
        };
        let text_style = *text_style;
        if text_style.wrap {
            return entity;
        }
        let Some(measured) = super::paint::measure_text(&entity, self.scale()) else {
            return entity;
        };
        let size = text_box_size(&text_style, measured);
        if (entity.transform.size.x - size.x).abs() >= 0.5
            || (entity.transform.size.y - size.y).abs() >= 0.5
        {
            entity.transform.size = size;
        }
        entity
    }

    pub(crate) fn context_position(&self) -> Option<CanvasPoint> {
        self.context_position.get()
    }

    pub(crate) fn note_context_position(&self, at: Option<CanvasPoint>) {
        self.context_position.set(at);
    }

    pub(crate) fn note_pointer(&self, at: Option<CanvasPoint>) {
        if self.pointer.get_untracked() != at {
            self.set_pointer.set(at);
        }
    }

    pub(crate) fn hold_pointer(&self, down: bool) {
        self.pointer_down.set(down);
    }

    pub(crate) fn pointer_held(&self) -> bool {
        self.pointer_down.get()
    }

    pub(crate) fn dismiss_import_error(&self) {
        self.set_import_error.set(None);
    }

    pub(crate) fn request_fit_selection(&self) {
        if let Some(bounds) = self.selected_bounds() {
            self.fit_into_view(bounds);
        }
    }

    pub(crate) fn request_fit_preview_region(&self) {
        if let Some(region) = self.preview_region.get_untracked() {
            self.fit_into_view(preview_region_bounds(region));
        }
    }

    pub(crate) fn selected_entities(&self) -> Vec<CanvasEntity> {
        let selection = self.selection.get();
        self.entities
            .get()
            .into_iter()
            .filter(|entity| selection.contains(&entity.id))
            .collect()
    }

    pub(crate) fn selected_unlocked(&self) -> Vec<CanvasEntity> {
        self.selected_entities()
            .into_iter()
            .filter(|entity| !entity.locked)
            .collect()
    }

    pub(crate) fn selection_has_unlocked(&self) -> bool {
        self.selected_entities().iter().any(|entity| !entity.locked)
    }

    fn selection_ids(entities: &[CanvasEntity], id: Uuid) -> HashSet<Uuid> {
        let group_id = entities
            .iter()
            .find(|entity| entity.id == id)
            .and_then(|entity| entity.group_id);
        entities
            .iter()
            .filter(|entity| match group_id {
                Some(group_id) => entity.group_id == Some(group_id),
                None => entity.id == id,
            })
            .map(|entity| entity.id)
            .collect()
    }

    pub(crate) fn select(&self, id: Uuid, additive: bool) {
        let entities = self.entities.get_untracked();
        let ids = Self::selection_ids(&entities, id);
        let mut selection = self.selection.get_untracked();
        if additive {
            if ids.iter().all(|id| selection.contains(id)) {
                selection.retain(|id| !ids.contains(id));
            } else {
                selection.extend(ids);
            }
        } else {
            if !ids.iter().all(|id| selection.contains(id)) {
                selection.clear();
            }
            selection.extend(ids);
        }
        self.set_selection.set(selection);
    }

    pub(crate) fn clear_selection(&self) {
        if !self.selection.get_untracked().is_empty() {
            self.set_selection.set(HashSet::new());
        }
    }

    pub(crate) fn selected_bounds(&self) -> Option<WorldRect> {
        self.selected_entities()
            .iter()
            .map(entity_bounds)
            .reduce(WorldRect::union)
    }

    pub(crate) fn entity_at(&self, point: CanvasPoint) -> Option<Uuid> {
        let radius = HIT_RADIUS / self.scale();
        self.entities
            .get_untracked()
            .iter()
            .rev()
            .find(|entity| hit_entity(entity, point, radius))
            .map(|entity| entity.id)
    }

    pub(crate) fn add_entity(&self, entity: CanvasEntity) {
        let id = entity.id;
        let text = matches!(&entity.kind, CanvasEntityKind::Text { .. });
        let select_after_add = !matches!(
            &entity.kind,
            CanvasEntityKind::Line | CanvasEntityKind::Rectangle | CanvasEntityKind::Pen { .. }
        );
        if text {
            self.edit_text(Some(id));
        }
        self.record(InfiniteCanvasOperation::Add { entity });
        let mut selection = HashSet::new();
        if select_after_add {
            selection.insert(id);
        }
        self.set_selection.set(selection);
    }

    pub(crate) fn duplicate_selection(&self) {
        let duplicates = duplicate_entities(
            self.selected_entities(),
            CanvasPoint::new(IMPORT_CASCADE_OFFSET, IMPORT_CASCADE_OFFSET),
        );
        if duplicates.is_empty() {
            return;
        }
        let mut selection = HashSet::new();
        for entity in duplicates {
            selection.insert(entity.id);
            self.operate(&InfiniteCanvasOperation::Add { entity });
        }
        self.set_selection.set(selection);
    }

    pub(crate) fn begin_move(&self, world: CanvasPoint, duplicate: bool) {
        let mut originals = self.selected_unlocked();
        if duplicate {
            originals = duplicate_entities(originals, CanvasPoint::default());
            self.set_selection
                .set(originals.iter().map(|entity| entity.id).collect());
        }
        self.begin_gesture((!originals.is_empty()).then_some(Gesture::Move {
            start: world,
            current: world,
            originals,
            duplicate,
        }));
    }

    pub(crate) fn selection_can_group(&self) -> bool {
        let selected = self.selected_entities();
        if selected.len() < 2 {
            return false;
        }
        let group = selected[0].group_id;
        group.is_none() || selected.iter().any(|entity| entity.group_id != group)
    }

    pub(crate) fn group_selection(&self) {
        if !self.selection_can_group() {
            return;
        }
        let before = self.selected_entities();
        let group_id = Uuid::new_v4();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                entity.group_id = Some(group_id);
                entity
            })
            .collect();
        self.record_update(before, after, false);
    }

    pub(crate) fn ungroup_selection(&self) {
        let before = self.selected_entities();
        let groups: HashSet<_> = before.iter().filter_map(|entity| entity.group_id).collect();
        if groups.is_empty() {
            return;
        }
        let before = self
            .entities
            .get_untracked()
            .into_iter()
            .filter(|entity| entity.group_id.is_some_and(|group| groups.contains(&group)))
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                entity.group_id = None;
                entity
            })
            .collect();
        self.record_update(before, after, false);
    }

    pub(crate) fn set_selection_locked(&self, locked: bool) {
        let before = self
            .selected_entities()
            .into_iter()
            .filter(|entity| entity.locked != locked)
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                entity.locked = locked;
                entity
            })
            .collect();
        self.record_update(before, after, false);
    }

    pub(crate) fn align_selection(&self, alignment: Alignment) {
        let before = self.selected_unlocked();
        if before.len() < 2 {
            return;
        }
        let bounds = before
            .iter()
            .map(entity_bounds)
            .reduce(WorldRect::union)
            .unwrap();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                let own = entity_bounds(&entity);
                let center = own.center();
                let (dx, dy) = match alignment {
                    Alignment::Left => (bounds.min.x - own.min.x, 0.0),
                    Alignment::HorizontalCenter => (bounds.center().x - center.x, 0.0),
                    Alignment::Right => (bounds.max.x - own.max.x, 0.0),
                    Alignment::Top => (0.0, bounds.min.y - own.min.y),
                    Alignment::VerticalCenter => (0.0, bounds.center().y - center.y),
                    Alignment::Bottom => (0.0, bounds.max.y - own.max.y),
                };
                entity.transform.center.x += dx;
                entity.transform.center.y += dy;
                entity
            })
            .collect();
        self.record_update(before, after, false);
    }

    pub(crate) fn distribute_selection(&self, horizontal: bool) {
        let before = self.selected_unlocked();
        if before.len() < 3 {
            return;
        }
        let mut ordered = before.clone();
        ordered.sort_by(|a, b| {
            let a = entity_bounds(a).center();
            let b = entity_bounds(b).center();
            let (a, b) = match horizontal {
                true => (a.x, b.x),
                false => (a.y, b.y),
            };
            a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
        });
        let first = entity_bounds(ordered.first().unwrap()).center();
        let last = entity_bounds(ordered.last().unwrap()).center();
        let span = match horizontal {
            true => last.x - first.x,
            false => last.y - first.y,
        };
        let step = span / (ordered.len() - 1) as f32;
        let after = ordered
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, mut entity)| {
                let center = entity_bounds(&entity).center();
                let wanted = match horizontal {
                    true => first.x + step * index as f32,
                    false => first.y + step * index as f32,
                };
                match horizontal {
                    true => entity.transform.center.x += wanted - center.x,
                    false => entity.transform.center.y += wanted - center.y,
                }
                entity
            })
            .collect();
        self.record_update(ordered, after, false);
    }

    pub(crate) fn can_reorder(&self, movement: CanvasLayerMove) -> bool {
        let entities = self.entities.get();
        let selection = self.selection.get();
        let selected: HashSet<_> = entities
            .iter()
            .filter(|entity| selection.contains(&entity.id) && !entity.locked)
            .map(|entity| entity.id)
            .collect();
        if selected.is_empty() {
            return false;
        }
        match movement {
            CanvasLayerMove::BringToFront | CanvasLayerMove::ForwardOne => {
                entities.iter().enumerate().any(|(index, entity)| {
                    selected.contains(&entity.id)
                        && entities[index + 1..]
                            .iter()
                            .any(|later| !selected.contains(&later.id))
                })
            }
            CanvasLayerMove::BackOne | CanvasLayerMove::SendToBack => {
                entities.iter().enumerate().any(|(index, entity)| {
                    selected.contains(&entity.id)
                        && entities[..index]
                            .iter()
                            .any(|earlier| !selected.contains(&earlier.id))
                })
            }
        }
    }

    pub(crate) fn reorder(&self, movement: CanvasLayerMove) {
        let ids = self
            .selected_unlocked()
            .iter()
            .map(|entity| entity.id)
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            self.record(InfiniteCanvasOperation::Reorder { ids, movement });
        }
    }

    pub(crate) fn run(&self, command: CanvasCommand) {
        match command {
            CanvasCommand::SelectAll => {
                let all = self
                    .entities
                    .get_untracked()
                    .iter()
                    .map(|entity| entity.id)
                    .collect();
                self.set_selection.set(all);
                self.set_tool(Tool::Select);
            }
            CanvasCommand::InvertSelection => {
                let selection = self.selection.get_untracked();
                let inverted = self
                    .entities
                    .get_untracked()
                    .iter()
                    .filter(|entity| !selection.contains(&entity.id))
                    .map(|entity| entity.id)
                    .collect();
                self.set_selection.set(inverted);
                self.set_tool(Tool::Select);
            }
            CanvasCommand::Duplicate => self.duplicate_selection(),
            CanvasCommand::Delete => {
                let ids = self
                    .selected_unlocked()
                    .iter()
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>();
                if !ids.is_empty() {
                    let removed: HashSet<_> = ids.iter().copied().collect();
                    let mut selection = self.selection.get_untracked();
                    selection.retain(|id| !removed.contains(id));
                    self.set_selection.set(selection);
                    self.record(InfiniteCanvasOperation::Remove { ids });
                }
            }
            CanvasCommand::Lock => self.set_selection_locked(true),
            CanvasCommand::Unlock => self.set_selection_locked(false),
            CanvasCommand::Group => self.group_selection(),
            CanvasCommand::Ungroup => self.ungroup_selection(),
            CanvasCommand::Reorder(movement) => self.reorder(movement),
            CanvasCommand::Copy => {
                self.copy_selection();
            }
            CanvasCommand::Cut => {
                if self.copy_selection() {
                    self.run(CanvasCommand::Delete);
                }
            }
            CanvasCommand::Paste => self.ask_to_paste(),
        }
    }

    fn copy_selection(&self) -> bool {
        let entities = self.selected_entities();
        if entities.is_empty() {
            return false;
        }
        match serde_json::to_string(&CanvasClipboardPayload { entities }) {
            Ok(json) => {
                block_editor_plugin::beui::reactive::copy_text(format!(
                    "{CANVAS_CLIPBOARD_PREFIX}{json}"
                ));
                true
            }
            Err(error) => {
                self.set_import_error
                    .set(Some(format!("Could not copy canvas objects: {error}")));
                false
            }
        }
    }

    pub(crate) fn ask_to_paste(&self) {
        block_editor_plugin::beui::reactive::request_paste();
        if self.focused_editor.get_untracked().is_none() {
            self.take_paste(true);
        }
    }

    pub(crate) fn paste_text(&self, text: &str) -> bool {
        let Some(json) = text.strip_prefix(CANVAS_CLIPBOARD_PREFIX) else {
            return false;
        };
        let payload = match serde_json::from_str::<CanvasClipboardPayload>(json) {
            Ok(payload) => payload,
            Err(error) => {
                self.set_import_error
                    .set(Some(format!("Could not paste canvas objects: {error}")));
                return true;
            }
        };
        let duplicates = duplicate_entities(
            payload.entities,
            CanvasPoint::new(IMPORT_CASCADE_OFFSET, IMPORT_CASCADE_OFFSET),
        );
        let mut selection = HashSet::new();
        for entity in duplicates {
            selection.insert(entity.id);
            self.operate(&InfiniteCanvasOperation::Add { entity });
        }
        self.set_selection.set(selection);
        self.set_tool(Tool::Select);
        true
    }

    pub(crate) fn open_block_picker(self: &Rc<Self>, center: Option<CanvasPoint>) {
        let state = Rc::downgrade(self);
        self.editor
            .pick_block(BlockFilter::default(), move |picked| {
                if let (Some(state), Ok(picked)) = (state.upgrade(), picked) {
                    state.place_picked_block(picked.id, center);
                }
            });
    }

    fn place_picked_block(&self, block_id: Uuid, center: Option<CanvasPoint>) {
        let center = center.unwrap_or_else(|| self.viewport_center());
        self.editor
            .blocks()
            .set_parent(block_id, BlockParent::Block(self.block_id()));
        self.add_direct_editor(block_id, center);
        self.set_tool(Tool::Select);
    }

    pub(crate) fn open_image_picker(&self, center: Option<CanvasPoint>) {
        self.pending_image_center.set(center);
        self.image_picker
            .borrow_mut()
            .open(self.editor.host(), image_filter());
    }

    pub(crate) fn open_component_picker(self: &Rc<Self>) {
        let selected = self
            .selected_entities()
            .iter()
            .map(|entity| entity.id)
            .collect::<HashSet<_>>();
        let state = Rc::downgrade(self);
        self.editor.pick_block(
            BlockFilter {
                name: "Component".to_owned(),
                block_types: vec![DatabaseSchemaContent::CONTENT_TYPE.into_bytes()],
                excluded: Vec::new(),
                templates: false,
            },
            move |picked| {
                if let (Some(state), Ok(picked)) = (state.upgrade(), picked) {
                    state.attach_picked_component(&selected, picked.id);
                }
            },
        );
    }

    fn attach_picked_component(&self, selected: &HashSet<Uuid>, reference: Uuid) {
        let before = self.entities_among(selected);
        let mut after = before.clone();
        attach_component(&mut after, selected, reference);
        self.record_update(before, after, false);
    }

    pub(crate) fn open_value_picker(
        self: &Rc<Self>,
        schema_id: Uuid,
        field_id: Uuid,
        block_type: Option<Uuid>,
    ) {
        let selected = self.selection.get_untracked();
        let filter = match block_type {
            Some(block_type) => BlockFilter {
                name: "Value".to_owned(),
                block_types: vec![block_type.into_bytes()],
                excluded: Vec::new(),
                templates: false,
            },
            None => BlockFilter::default(),
        };
        let state = Rc::downgrade(self);
        self.editor.pick_block(filter, move |picked| {
            if let (Some(state), Ok(picked)) = (state.upgrade(), picked) {
                state.set_picked_value(&selected, schema_id, field_id, picked.id);
            }
        });
    }

    fn set_picked_value(
        &self,
        selected: &HashSet<Uuid>,
        schema_id: Uuid,
        field_id: Uuid,
        reference: Uuid,
    ) {
        let before = self.entities_among(selected);
        let mut after = before.clone();
        set_component_value(
            &mut after,
            selected,
            schema_id,
            field_id,
            Some(DatabaseValue::Block(reference)),
        );
        self.record_update(before, after, false);
    }

    fn entities_among(&self, selected: &HashSet<Uuid>) -> Vec<CanvasEntity> {
        self.entities
            .get_untracked()
            .into_iter()
            .filter(|entity| selected.contains(&entity.id))
            .collect()
    }

    pub(crate) fn update_selected(
        &self,
        compatible: impl Fn(&CanvasEntityKind) -> bool,
        mut update: impl FnMut(&mut CanvasEntityStyle),
    ) {
        let before = self
            .selected_entities()
            .into_iter()
            .filter(|entity| compatible(&entity.kind))
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                update(&mut entity.style);
                entity
            })
            .collect();
        self.record_update(before, after, true);
    }

    pub(crate) fn update_selected_text(&self, mut update: impl FnMut(&mut CanvasTextStyle)) {
        let before = self
            .selected_entities()
            .into_iter()
            .filter(|entity| matches!(entity.kind, CanvasEntityKind::Text { .. }))
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                let CanvasEntityKind::Text { text_style, .. } = &mut entity.kind else {
                    unreachable!("only text entities were kept");
                };
                update(text_style);
                entity
            })
            .collect();
        self.record_update(before, after, true);
    }

    pub(crate) fn set_preview_region(&self, region: Option<CanvasPreviewRegion>) {
        self.record(InfiniteCanvasOperation::SetPreviewRegion { region });
    }

    pub(crate) fn set_preview_region_grouped(&self, region: CanvasPreviewRegion) {
        self.grouped_edit.set(true);
        self.operate(&InfiniteCanvasOperation::SetPreviewRegion {
            region: Some(region),
        });
    }

    pub(crate) fn replace_referenced_block(&self, old: Uuid, new: Uuid) -> bool {
        let replaced = self
            .entities
            .get_untracked()
            .into_iter()
            .filter_map(|entity| {
                let kind = match entity.kind {
                    CanvasEntityKind::Block { block_id } if block_id == old => {
                        CanvasEntityKind::Block { block_id: new }
                    }
                    CanvasEntityKind::DirectEditor { block_id, scale } if block_id == old => {
                        CanvasEntityKind::DirectEditor {
                            block_id: new,
                            scale,
                        }
                    }
                    _ => return None,
                };
                Some(CanvasEntity { kind, ..entity })
            })
            .collect::<Vec<_>>();
        if replaced.is_empty() {
            return false;
        }
        self.record(InfiniteCanvasOperation::Update { entities: replaced });
        true
    }
}

pub(crate) fn selection_frame(
    entities: &[CanvasEntity],
    selection: &HashSet<Uuid>,
) -> Option<SelectionFrame> {
    let selected = entities
        .iter()
        .filter(|entity| selection.contains(&entity.id))
        .collect::<Vec<_>>();
    match selected.as_slice() {
        [] => None,
        [entity] => Some(SelectionFrame {
            center: entity.transform.center,
            size: entity.transform.size,
            rotation: entity.transform.rotation,
        }),
        _ => selected
            .into_iter()
            .map(entity_bounds)
            .reduce(WorldRect::union)
            .map(SelectionFrame::from_world_rect),
    }
}

pub(crate) fn attach_component(
    entities: &mut [CanvasEntity],
    selected: &HashSet<Uuid>,
    schema_id: Uuid,
) {
    for entity in entities {
        if selected.contains(&entity.id)
            && !entity
                .components
                .iter()
                .any(|component| component.schema_id == schema_id)
        {
            entity.components.push(CanvasComponent {
                schema_id,
                values: BTreeMap::new(),
            });
        }
    }
}

pub(crate) fn set_component_value(
    entities: &mut [CanvasEntity],
    selected: &HashSet<Uuid>,
    schema_id: Uuid,
    field_id: Uuid,
    value: Option<DatabaseValue>,
) {
    for entity in entities {
        if !selected.contains(&entity.id) {
            continue;
        }
        let Some(component) = entity
            .components
            .iter_mut()
            .find(|component| component.schema_id == schema_id)
        else {
            continue;
        };
        match value.clone() {
            Some(value) => component.values.insert(field_id, value),
            None => component.values.remove(&field_id),
        };
    }
}

pub(crate) fn remove_component(
    entities: &mut [CanvasEntity],
    selected: &HashSet<Uuid>,
    schema_id: Uuid,
) {
    for entity in entities {
        if selected.contains(&entity.id) {
            entity
                .components
                .retain(|component| component.schema_id != schema_id);
        }
    }
}

impl CanvasState {
    pub(crate) fn edit_components(
        &self,
        edit: impl FnOnce(&mut Vec<CanvasEntity>, &HashSet<Uuid>),
    ) {
        let selection = self.selection.get_untracked();
        let before = self.selected_entities();
        let mut after = before.clone();
        edit(&mut after, &selection);
        self.record_update(before, after, false);
    }

    pub(crate) fn selection_defaults_to_proportional(&self) -> bool {
        self.selected_unlocked().iter().any(|entity| {
            matches!(
                entity.kind,
                CanvasEntityKind::Block { .. } | CanvasEntityKind::DirectEditor { .. }
            ) && self
                .peek_child(entity.id)
                .capabilities
                .preserve_aspect_ratio
        })
    }

    pub(crate) fn selection_forces_proportional(&self) -> bool {
        self.selected_unlocked().iter().any(|entity| {
            matches!(entity.kind, CanvasEntityKind::DirectEditor { .. })
                && self
                    .peek_child(entity.id)
                    .capabilities
                    .preserve_aspect_ratio
        })
    }

    pub(crate) fn selection_allows_rotation(&self) -> bool {
        self.selected_unlocked()
            .iter()
            .all(|entity| match entity.kind {
                CanvasEntityKind::DirectEditor { .. } => {
                    let state = self.peek_child(entity.id);
                    !state.placed || state.capabilities.rotation
                }
                _ => true,
            })
    }

    pub(crate) fn selection_resize(&self) -> ResizeMode {
        let selected = self.selected_unlocked();
        if selected.is_empty() {
            return ResizeMode::None;
        }
        let mut horizontal = true;
        let mut vertical = true;
        for entity in &selected {
            let resize = self.resize_mode(entity);
            horizontal &= resize.horizontal();
            vertical &= resize.vertical();
        }
        match (horizontal, vertical) {
            (true, true) => ResizeMode::Both,
            (true, false) => ResizeMode::Horizontal,
            (false, true) => ResizeMode::Vertical,
            (false, false) => ResizeMode::None,
        }
    }

    pub(crate) fn selection_handles(&self) -> (ResizeMode, bool) {
        match self.selection_resize() {
            ResizeMode::None if self.selection_has_unlocked() => (ResizeMode::Both, true),
            resize => (resize, false),
        }
    }

    pub(crate) fn add_direct_editor(&self, block_id: Uuid, center: CanvasPoint) {
        self.add_direct_editor_sized(block_id, center, CanvasPoint::new(180.0, 100.0));
    }

    pub(crate) fn add_direct_editor_sized(
        &self,
        block_id: Uuid,
        center: CanvasPoint,
        content_size: CanvasPoint,
    ) {
        let size = direct_editor_entity_size(Vec2::new(content_size.x, content_size.y), 1.0);
        let entity_id = Uuid::new_v4();
        let transform = CanvasTransform::new(center, size, 0.0);
        self.set_selection.set(HashSet::from([entity_id]));
        self.record(InfiniteCanvasOperation::Add {
            entity: CanvasEntity {
                id: entity_id,
                transform,
                kind: CanvasEntityKind::DirectEditor {
                    block_id,
                    scale: 1.0,
                },
                style: CanvasEntityStyle::default(),
                group_id: None,
                locked: false,
                components: Vec::new(),
            },
        });
    }

    pub(crate) fn add_imported_image(&self, image: ImageContent, center: CanvasPoint) {
        let id = self.editor.create_child(&image);
        self.add_direct_editor(id, center);
    }

    pub(crate) fn toggle_selected_block_mode(&self) {
        let selected = self.selected_entities();
        let [entity] = selected.as_slice() else {
            return;
        };
        let updated = match entity.kind {
            CanvasEntityKind::DirectEditor { block_id, .. } => {
                if self.focused_editor.get_untracked() == Some(entity.id) {
                    self.focus_editor(None);
                }
                direct_editor_to_preview(entity, block_id)
            }
            CanvasEntityKind::Block { block_id } => self
                .peek_child(entity.id)
                .intrinsic_size
                .map(|intrinsic| preview_to_direct_editor(entity, block_id, intrinsic)),
            _ => None,
        };
        if let Some(updated) = updated {
            self.record_update(vec![entity.clone()], vec![updated], false);
        }
    }

    pub(crate) fn edit_selected(&self) {
        let selected = self.selected_entities();
        let [entity] = selected.as_slice() else {
            return;
        };
        match entity.kind {
            CanvasEntityKind::DirectEditor { block_id, .. } => {
                if self.interaction(entity.id) == Some(InteractionMode::Playback) {
                    self.open_referenced_block(block_id);
                } else {
                    self.focus_editor(Some(entity.id));
                }
            }
            CanvasEntityKind::Text { .. } if !entity.locked => {
                self.edit_text(Some(entity.id));
            }
            CanvasEntityKind::Block { block_id } => self.open_referenced_block(block_id),
            _ => {}
        }
    }

    pub(crate) fn open_referenced_block(&self, id: Uuid) {
        let Some(block_type) = self.block_type_of(id) else {
            return;
        };
        self.editor.host().open_block(id, block_type);
    }

    pub(crate) fn finish_gesture(&self, gesture: Gesture) {
        match gesture {
            Gesture::Create {
                tool,
                start,
                current,
                from_center,
                ..
            } => {
                if let Some(entity) = self.created_entity(tool, start, current, from_center) {
                    let text = matches!(entity.kind, CanvasEntityKind::Text { .. });
                    self.add_entity(entity);
                    if text {
                        self.set_tool(Tool::Select);
                    }
                }
            }
            Gesture::Pen { points } => {
                if points.len() >= 2 {
                    self.add_entity(pen_entity(points, self.default_style()));
                }
            }
            Gesture::SelectBox {
                start,
                current,
                from_center,
                additive,
                ..
            } => {
                let mut selection = match additive {
                    true => self.selection.get_untracked(),
                    false => HashSet::new(),
                };
                if distance(start, current) >= HIT_RADIUS / self.scale() {
                    let entities = self.entities.get_untracked();
                    let box_rect = gesture_rect(start, current, from_center);
                    let hits = entities
                        .iter()
                        .filter(|entity| box_rect.contains_rect(entity_bounds(entity)))
                        .map(|entity| entity.id)
                        .collect::<Vec<_>>();
                    for id in hits {
                        selection.extend(Self::selection_ids(&entities, id));
                    }
                }
                self.set_selection.set(selection);
            }
            Gesture::Move {
                duplicate: true, ..
            } => {
                let additions = preview_entities(&gesture);
                for entity in additions {
                    self.operate(&InfiniteCanvasOperation::Add { entity });
                }
            }
            Gesture::Move {
                ref originals,
                duplicate: false,
                ..
            }
            | Gesture::Resize { ref originals, .. }
            | Gesture::Rotate { ref originals, .. } => {
                let updates = preview_entities(&gesture);
                if !updates.is_empty() {
                    self.record_update(originals.clone(), updates, false);
                }
            }
        }
    }

    fn created_entity(
        &self,
        tool: Tool,
        start: CanvasPoint,
        current: CanvasPoint,
        from_center: bool,
    ) -> Option<CanvasEntity> {
        let style = self.default_style();
        match tool {
            Tool::Line if distance(start, current) >= MIN_SIZE => {
                let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
                Some(CanvasEntity {
                    id: Uuid::new_v4(),
                    transform: CanvasTransform::new(
                        midpoint(start, current),
                        CanvasPoint::new(distance(start, current), 1.0),
                        delta.y.atan2(delta.x),
                    ),
                    kind: CanvasEntityKind::Line,
                    style,
                    group_id: None,
                    locked: false,
                    components: Vec::new(),
                })
            }
            Tool::Rectangle => {
                let bounds = gesture_rect(start, current, from_center);
                Some(CanvasEntity {
                    id: Uuid::new_v4(),
                    transform: CanvasTransform::new(
                        bounds.center(),
                        CanvasPoint::new(
                            bounds.size().x.max(MIN_SIZE),
                            bounds.size().y.max(MIN_SIZE),
                        ),
                        0.0,
                    ),
                    kind: CanvasEntityKind::Rectangle,
                    style,
                    group_id: None,
                    locked: false,
                    components: Vec::new(),
                })
            }
            Tool::Text => {
                let bounds = WorldRect::from_points(start, current);
                let dragged = distance(start, current) >= MIN_SIZE;
                let size = match dragged {
                    true => CanvasPoint::new(bounds.size().x.max(60.0), bounds.size().y.max(32.0)),
                    false => CanvasPoint::new(180.0, 36.0),
                };
                Some(CanvasEntity {
                    id: Uuid::new_v4(),
                    transform: CanvasTransform::new(
                        if dragged { bounds.center() } else { start },
                        size,
                        0.0,
                    ),
                    kind: CanvasEntityKind::Text {
                        text: String::new(),
                        text_style: CanvasTextStyle {
                            wrap: dragged,
                            ..CanvasTextStyle::default()
                        },
                        placeholder: "Text".into(),
                    },
                    style,
                    group_id: None,
                    locked: false,
                    components: Vec::new(),
                })
            }
            Tool::Line | Tool::Select | Tool::Pen => None,
        }
    }
}

impl CanvasState {
    pub(crate) fn watch(self: &Rc<Self>) {
        let state = Rc::clone(self);
        create_effect(move || state.publish_references());
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
        self.editor.on_reply(move || {
            state.take_image_pick();
            state.take_paste(false);
        });
        if self.preview {
            return;
        }
        let state = Rc::clone(self);
        create_effect(move || {
            state.entities.with(|_| ());
            state.child_states.with(|_| ());
            untrack(|| state.autosize_direct_editors());
        });
        let state = Rc::clone(self);
        let visible = self.editor.presence_visible();
        create_effect(move || {
            visible.with(|_| ());
            state.pointer.with(|_| ());
            state.selection.with(|_| ());
            untrack(|| state.publish_presence());
        });
        let state = Rc::clone(self);
        create_effect(move || {
            state.entities.with(|_| ());
            state.peers.with(|_| ());
            untrack(|| state.read_presence());
        });
        let state = Rc::clone(self);
        let resized = self.editor.resized();
        create_effect(move || {
            let Some(size) = resized.get() else {
                return;
            };
            untrack(|| state.resize_preview_region(size));
        });
        let state = Rc::clone(self);
        let revealed = self.editor.revealed();
        create_effect(move || {
            let Some(client_id) = revealed.get() else {
                return;
            };
            untrack(|| state.reveal_peer(client_id));
        });
    }

    fn publish_references(&self) {
        let catalog = self.types();
        let dependencies = self.dependencies.read();
        let labels: HashMap<Uuid, BlockLabel> = dependencies
            .iter()
            .map(|reference| (reference.id, reference.label(catalog.as_ref())))
            .collect();
        let types: HashMap<Uuid, Uuid> = dependencies
            .iter()
            .map(|reference| (reference.id, reference.block_type))
            .collect();
        let thumbhashes: HashMap<Uuid, Thumbhash> = dependencies
            .iter()
            .filter_map(|reference| Some((reference.id, reference.thumbhash.clone()?)))
            .collect();
        untrack(|| {
            if self.thumbhashes.get_untracked() != thumbhashes {
                self.set_thumbhashes.set(thumbhashes);
            }
            if self.labels.get_untracked() != labels {
                self.set_labels.set(labels);
            }
            if self.types.get_untracked() != types {
                self.set_types.set(types);
            }
        });
    }

    fn take_image_pick(&self) {
        let picked = self
            .image_picker
            .borrow_mut()
            .poll(self.editor.host())
            .map(|file| file.map(imported_image));
        match picked {
            Some(Ok(image)) => {
                self.set_import_error.set(None);
                let center = self
                    .pending_image_center
                    .take()
                    .unwrap_or_else(|| self.viewport_center());
                self.add_imported_image(image, center);
                self.set_tool(Tool::Select);
            }
            Some(Err(error)) => {
                self.pending_image_center.set(None);
                self.set_import_error.set(Some(error));
            }
            None => {}
        }
    }

    fn take_drag(&self, drag: Option<Drag>) {
        let Some(drag) = drag else {
            return;
        };
        if drag.block_id == self.block_id() || self.focused_editor.get_untracked().is_some() {
            return;
        }
        self.editor.accept_drag(true);
        if !drag.dropped {
            return;
        }
        let center = self.world_at(drag.position);
        self.editor
            .blocks()
            .set_parent(drag.block_id, BlockParent::Block(self.block_id()));
        self.add_direct_editor(drag.block_id, center);
    }

    fn take_files(&self, drop: Option<FileDrop>) {
        if self.focused_editor.get_untracked().is_some() {
            return;
        }
        let Some(drop) = drop else {
            self.pending_file_drop.set(None);
            return;
        };
        let at = self.world_at(drop.position);
        if !drop.dropped {
            self.pending_file_drop.set(Some(at));
            return;
        }
        self.set_import_error.set(None);
        let base = self.pending_file_drop.take().unwrap_or(at);
        for (index, file) in drop.files.into_iter().enumerate() {
            let offset = IMPORT_CASCADE_OFFSET * index as f32;
            self.add_imported_image(
                ImageContent::from_file(file.name, file.data),
                CanvasPoint::new(base.x + offset, base.y + offset),
            );
        }
    }

    fn take_paste(&self, asked: bool) {
        let pasted = self.paster.borrow_mut().paste(self.editor.host(), asked);
        match pasted {
            Some(PastedImage::Image { name, data }) => {
                self.set_import_error.set(None);
                let center = self.viewport_center();
                self.add_imported_image(ImageContent::from_file(name, data), center);
            }
            Some(PastedImage::Failed(error)) => self.set_import_error.set(Some(error)),
            Some(PastedImage::Empty) | None => {}
        }
    }

    fn publish_presence(&self) {
        let cursor = self.editor.presence_visible().get_untracked().then(|| {
            let color = self.color.get().unwrap_or_else(|| {
                let used = self
                    .peers
                    .get_untracked()
                    .into_iter()
                    .map(|(_, cursor)| cursor.color);
                let color = pick_free_color(used);
                self.color.set(Some(color));
                color
            });
            CanvasCursor {
                pointer: self.pointer.get_untracked(),
                selection: self.selection.get_untracked().iter().copied().collect(),
                color,
            }
        });
        if *self.shown.borrow() == cursor {
            return;
        }
        self.editor.show(cursor.as_ref());
        self.shown.replace(cursor);
    }

    fn read_presence(&self) {
        let entities = self.entities.get_untracked();
        let mut presence = Presence::default();
        for (_, cursor) in self.peers.get_untracked() {
            let color = cursor.color;
            if !cursor.selection.is_empty() {
                let selected: HashSet<_> = cursor.selection.iter().copied().collect();
                if let Some(bounds) = entities
                    .iter()
                    .filter(|entity| selected.contains(&entity.id))
                    .map(entity_bounds)
                    .reduce(WorldRect::union)
                {
                    presence.selections.push(RemoteSelection {
                        frame: SelectionFrame::from_world_rect(bounds),
                        color,
                    });
                }
            }
            if cursor.pointer.is_some() {
                presence.cursors.push(RemoteCursor {
                    pointer: cursor.pointer,
                    color,
                });
            }
        }
        if self.presence.get_untracked() != presence {
            self.set_presence.set(presence);
        }
    }

    fn resize_preview_region(&self, size: Vec2) {
        let Some(region) = self.preview_region.get_untracked() else {
            return;
        };
        let updated = CanvasPreviewRegion::new(
            region.center,
            CanvasPoint::new(size.x.max(MIN_SIZE), size.y.max(MIN_SIZE)),
        );
        if (updated.size.x - region.size.x).abs() < 0.01
            && (updated.size.y - region.size.y).abs() < 0.01
        {
            return;
        }
        self.set_preview_region(Some(updated));
    }

    fn reveal_peer(&self, client_id: u64) {
        let Some((_, cursor)) = self
            .peers
            .get_untracked()
            .into_iter()
            .find(|(id, _)| *id == client_id)
        else {
            return;
        };
        let entities = self.entities.get_untracked();
        let target = cursor.pointer.or_else(|| {
            entities
                .iter()
                .filter(|entity| cursor.selection.contains(&entity.id))
                .map(entity_bounds)
                .reduce(WorldRect::union)
                .map(|bounds| bounds.center())
        });
        if let Some(target) = target {
            self.editor
                .reveal(Rect::from_min_max(point(target), point(target)));
        }
    }

    fn fit_into_view(&self, bounds: WorldRect) {
        let stage = self.stage.get();
        let available = (stage.size() - Vec2::splat(40.0)).max(Vec2::splat(1.0));
        let target = bounds.rect();
        let scale = self.scale();
        let factor = (available.x / (target.width() * scale).max(1.0))
            .min(available.y / (target.height() * scale).max(1.0));
        let anchor = self.camera().to_screen(target.center());
        self.editor.zoom_at(factor, anchor);
        self.editor.pan(stage.center() - anchor);
    }

    pub(crate) fn apply_view_change(&self, entity: Uuid, change: block_editor_plugin::ViewChange) {
        match change {
            block_editor_plugin::ViewChange::Fit => self.request_fit_entity(entity),
            block_editor_plugin::ViewChange::Pan { x, y } => {
                self.editor.pan(Vec2::new(x, y));
            }
            block_editor_plugin::ViewChange::Zoom { factor, anchor } => match anchor {
                Some((x, y)) => self.editor.zoom_at(factor, Pos2::new(x, y)),
                None => self.editor.zoom(factor),
            },
            block_editor_plugin::ViewChange::ResumeAutoFit => self.editor.resume_auto_fit(),
        }
    }

    pub(crate) fn request_fit_entity(&self, entity: Uuid) {
        if let Some(bounds) = self
            .entities
            .get_untracked()
            .iter()
            .find(|candidate| candidate.id == entity)
            .and_then(|candidate| direct_editor_layout(candidate).map(|layout| layout.content))
        {
            self.fit_into_view(bounds);
        }
    }

    fn autosize_direct_editors(&self) {
        let mut before = Vec::new();
        let mut after = Vec::new();
        for entity in self.entities.get_untracked() {
            let CanvasEntityKind::DirectEditor { scale, .. } = entity.kind else {
                continue;
            };
            let child = self.peek_child(entity.id);
            if !child.placed {
                continue;
            }
            let intrinsic = match child.resize {
                ResizeMode::Horizontal => {
                    let width = direct_editor_layout(&entity)
                        .map(|layout| layout.content.size().x / scale.max(f32::EPSILON))
                        .unwrap_or(MIN_SIZE);
                    child
                        .intrinsic_size
                        .map(|intrinsic| Vec2::new(width, intrinsic.y))
                }
                _ => child.intrinsic_size,
            };
            let Some(intrinsic) = intrinsic else {
                continue;
            };
            let intrinsic_size = direct_editor_entity_size(intrinsic, scale);
            let desired = CanvasPoint::new(
                match child.resize.horizontal() {
                    true => entity.transform.size.x,
                    false => intrinsic_size.x,
                },
                match child.resize.vertical() {
                    true => entity.transform.size.y,
                    false => intrinsic_size.y,
                },
            );
            if (entity.transform.size.x - desired.x).abs() < 0.01
                && (entity.transform.size.y - desired.y).abs() < 0.01
                && entity.transform.rotation == 0.0
            {
                continue;
            }
            let bounds = entity_bounds(&entity);
            let mut updated = entity.clone();
            updated.transform.size = desired;
            updated.transform.rotation = 0.0;
            updated.transform.center = CanvasPoint::new(
                bounds.min.x + desired.x * 0.5,
                bounds.min.y + desired.y * 0.5,
            );
            before.push(entity);
            after.push(updated);
        }
        self.record_update(before, after, true);
    }

    pub(crate) fn assigned_intrinsic(&self, entity: &CanvasEntity) -> Option<Vec2> {
        let CanvasEntityKind::DirectEditor { scale, .. } = entity.kind else {
            return None;
        };
        if self.peek_child(entity.id).resize != ResizeMode::Both {
            return None;
        }
        let content = direct_editor_layout(entity)?.content.size();
        Some(Vec2::new(
            content.x / scale.max(f32::EPSILON),
            content.y / scale.max(f32::EPSILON),
        ))
    }
}
