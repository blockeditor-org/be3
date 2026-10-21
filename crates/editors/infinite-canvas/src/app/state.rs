use block_editor_beui::be_block::{BlockContent, DatabaseSchemaContent};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::presence::CanvasCursor;
use block_editor_beui::ContentProjection;
use block_editor_beui::be_block::CanvasContent;
use block_editor_beui::be_block::Edit;
use block_editor_beui::be_block::ImageContent;
use block_editor_beui::be_block::canvas::{
    CanvasColor, CanvasComponent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle,
    CanvasLayerMove, CanvasPoint, CanvasTextStyle, CanvasTransform, InfiniteCanvasOperation,
    first_artboard,
};
use block_editor_beui::be_block::database::DatabaseValue;
use block_editor_beui::be_block::presence::{PresenceColor, pick_free_color};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{Pos2, Rect, Thumbhash, Vec2, pos2};
use block_editor_beui::block_ui::BlockLabel;
use block_editor_beui::database::DatabaseBlockPickRequest;
use block_editor_beui::{
    BlockFilter, ChildRect, ChildState, Drag, Editor, FileDrop, InteractionMode, ResizeMode,
    child_rect,
};
use block_editor_beui::{BlockParent, BlockQuery};
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
    Artboard,
    Line,
    Rectangle,
    Text,
    Pen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TextEntry {
    At(Pos2),
    All,
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
        pressures: Vec<f32>,
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
        grab: CanvasPoint,
        originals: Vec<CanvasEntity>,
        default_preserve_aspect_ratio: bool,
        force_preserve_aspect_ratio: bool,
        preserve_aspect_ratio: bool,
        scale_text: bool,
        scale_editors: bool,
    },
    Endpoint {
        original: CanvasEntity,
        end: usize,
        current: CanvasPoint,
        grab: CanvasPoint,
        snap: bool,
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
        Gesture::Endpoint {
            original,
            end,
            current,
            snap,
            ..
        } => {
            let mut ends = line_ends(original);
            ends[*end] = match snap {
                true => constrain_point_angle(ends[1 - end], *current, std::f32::consts::FRAC_PI_4),
                false => *current,
            };
            vec![line_through(original.clone(), ends[0], ends[1])]
        }
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

#[derive(Clone)]
struct Placeholder {
    entity: CanvasEntity,
    fitted: bool,
}

struct CanvasCells {
    content: ContentProjection<CanvasContent>,
    pending_image_center: Cell<Option<CanvasPoint>>,
    pending_file_drop: Cell<Option<CanvasPoint>>,
    context_position: Cell<Option<CanvasPoint>>,
    grouped_edit: Cell<bool>,
    pointer_down: Cell<bool>,
    touch: Cell<bool>,
    retapped: Cell<Option<(Uuid, Pos2)>>,
    text_entry: Cell<Option<TextEntry>>,
    surface: NodeRef,
    last_foreground: Cell<CanvasColor>,
    last_fill: Cell<Option<CanvasColor>>,
    children: RefCell<HashMap<Uuid, ChildState>>,
    placeholders: RefCell<HashMap<Uuid, Placeholder>>,
    confirmed_editor: Cell<Option<Uuid>>,
    shown: RefCell<Option<CanvasCursor>>,
    color: Cell<Option<PresenceColor>>,
    stage: Cell<Rect>,
}

#[derive(Clone, Copy)]
pub(crate) struct CanvasState {
    cells: Stored<CanvasCells>,
    editor: Editor,
    preview: bool,
    pub(crate) pointer: Signal<Option<CanvasPoint>>,
    pub(crate) entities: ReadSignal<Vec<CanvasEntity>>,
    pub(crate) tool: Signal<Tool>,
    pub(crate) selection: Signal<HashSet<Uuid>>,
    pub(crate) gesture: Signal<Option<Gesture>>,
    typed: Signal<Option<CanvasEntity>>,
    pub(crate) focused_editor: Signal<Option<Uuid>>,
    pub(crate) editing_text: Signal<Option<Uuid>>,
    pub(crate) import_error: Signal<Option<String>>,
    labels: Memo<Ready<HashMap<Uuid, BlockLabel>>>,
    types: Memo<HashMap<Uuid, Uuid>>,
    thumbhashes: Memo<HashMap<Uuid, Thumbhash>>,
    pub(crate) child_states: Signal<HashMap<Uuid, ChildState>>,
    pub(crate) presence: Signal<Presence>,
    peers: ReadSignal<Vec<(u64, CanvasCursor)>>,
}

impl CanvasState {
    pub(crate) fn new(editor: Editor, preview: bool) -> Self {
        let content = editor.block_content::<CanvasContent>();
        let entities = content.project(|canvas| canvas.root().entities());
        let tool = create_signal(Tool::Select);
        let selection = create_signal(HashSet::new());
        let gesture = create_signal(None);
        let typed = create_signal(None);
        let focused_editor = create_signal(None);
        let editing_text = create_signal(None);
        let import_error = create_signal(None);
        let dependencies = editor
            .blocks()
            .watch(BlockQuery::References(editor.block_id()));
        let references = create_memo(move || dependencies.read());
        let labels = create_memo(move || {
            let catalog = editor.block_types();
            Ok(references
                .get()?
                .iter()
                .map(|reference| (reference.id, reference.label(catalog.as_ref())))
                .collect())
        });
        let types = create_memo(move || {
            references.with(|references| {
                references
                    .iter()
                    .flatten()
                    .map(|reference| (reference.id, reference.block_type))
                    .collect()
            })
        });
        let thumbhashes = create_memo(move || {
            references.with(|references| {
                references
                    .iter()
                    .flatten()
                    .filter_map(|reference| {
                        let thumbhash = reference.thumbhash()?;
                        Some((
                            reference.id,
                            Thumbhash {
                                hash: thumbhash.hash,
                                width: thumbhash.width,
                                height: thumbhash.height,
                            },
                        ))
                    })
                    .collect()
            })
        });
        let child_states = create_signal(HashMap::new());
        let presence = create_signal(Presence::default());
        let pointer = create_signal(None);
        let cells = store(CanvasCells {
            content,
            pending_image_center: Cell::new(None),
            pending_file_drop: Cell::new(None),
            context_position: Cell::new(None),
            grouped_edit: Cell::new(false),
            pointer_down: Cell::new(false),
            touch: Cell::new(false),
            retapped: Cell::new(None),
            text_entry: Cell::new(None),
            surface: NodeRef::new(),
            last_foreground: Cell::new(CanvasEntityStyle::default().foreground),
            last_fill: Cell::new(CanvasEntityStyle::default().fill),
            children: RefCell::new(HashMap::new()),
            placeholders: RefCell::new(HashMap::new()),
            confirmed_editor: Cell::new(None),
            shown: RefCell::new(None),
            color: Cell::new(None),
            stage: Cell::new(Rect::ZERO),
        });
        Self {
            cells,
            editor,
            preview,
            pointer,
            entities,
            tool,
            selection,
            gesture,
            typed,
            focused_editor,
            editing_text,
            import_error,
            labels,
            types,
            thumbhashes,
            child_states,
            presence,
            peers: editor.peers::<CanvasCursor>(),
        }
    }

    pub(crate) fn is_alive(&self) -> bool {
        self.cells.is_alive()
    }

    pub(crate) fn set_stage(&self, stage: Rect) {
        self.cells.value().stage.set(stage);
    }

    pub(crate) fn editor(&self) -> Editor {
        self.editor
    }

    pub(crate) fn previewing(&self) -> bool {
        self.preview
    }

    pub(crate) fn block_id(&self) -> Uuid {
        self.editor.block_id()
    }

    pub(crate) fn camera(&self) -> CanvasView {
        camera_view(
            self.editor.canvas().get_untracked(),
            self.editor.world().get_untracked(),
            self.cells.value().stage.get(),
            self.framed(),
        )
    }

    pub(crate) fn framed(&self) -> Option<WorldRect> {
        self.preview.then(|| embedded_region(&self.entities.get()))
    }

    pub(crate) fn scale(&self) -> f32 {
        self.camera().scale.max(f32::EPSILON)
    }

    pub(crate) fn world_at(&self, at: Pos2) -> CanvasPoint {
        let canvas = self.camera().to_canvas(at);
        CanvasPoint::new(canvas.x, canvas.y)
    }

    pub(crate) fn visible_world(&self) -> Rect {
        self.camera().rect_to_canvas(self.cells.value().stage.get())
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
            self.typed.set(entity);
        }
    }

    pub(crate) fn label_of(&self, reference: Uuid) -> Option<BlockLabel> {
        self.labels
            .with(|labels| labels.as_ref().ok()?.get(&reference).cloned())
    }

    pub(crate) fn unlabeled(&self) -> String {
        self.labels.with(|labels| match labels {
            Err(Pending::Failed(error)) => error.clone(),
            _ => "Loading…".to_owned(),
        })
    }

    pub(crate) fn thumbhash_of(&self, reference: Uuid) -> Option<Thumbhash> {
        self.thumbhashes
            .with(|thumbhashes| thumbhashes.get(&reference).cloned())
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
        let released = self
            .cells
            .value()
            .children
            .borrow()
            .get(&entity)
            .is_some_and(|held| held.active && !state.active);
        if released && self.focused_editor.get_untracked() == Some(entity) {
            self.focus_editor(None);
        }
        self.cells
            .value()
            .children
            .borrow_mut()
            .insert(entity, state);
        let held = self.cells.value().children.borrow().clone();
        self.child_states.set(held);
    }

    fn peek_child(&self, entity: Uuid) -> ChildState {
        self.cells
            .value()
            .children
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

    pub(crate) fn takes_frame(&self, entity: Uuid) -> bool {
        let child = self.peek_child(entity);
        child.interaction.is_some() && child.capabilities.pan_and_zoom
    }

    pub(crate) fn works_in_place(&self, entity: Uuid) -> bool {
        let child = self.child_state(entity);
        child.interaction == Some(InteractionMode::Live) && !child.capabilities.pan_and_zoom
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
    framed: Option<WorldRect>,
) -> CanvasView {
    let centre = framed.map_or_else(CanvasPoint::default, WorldRect::center);
    let Some(view) = view else {
        let scale = framed.map_or(1.0, |framed| fit_scale(stage, framed));
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

const FITTED_ROOM: Vec2 = Vec2::new(640.0, 480.0);

fn fit_to_child(entity: &CanvasEntity, intrinsic: Vec2) -> Option<CanvasEntity> {
    let CanvasEntityKind::DirectEditor { block_id, .. } = entity.kind else {
        return None;
    };
    if intrinsic.x <= 0.0 || intrinsic.y <= 0.0 {
        return None;
    }
    let room = direct_editor_layout(entity)?.content.size();
    let room = Vec2::new(room.x.max(FITTED_ROOM.x), room.y.max(FITTED_ROOM.y));
    let scale = (room.x / intrinsic.x)
        .min(room.y / intrinsic.y)
        .clamp(f32::EPSILON, 1.0);
    let bounds = entity_bounds(entity);
    let size = direct_editor_entity_size(intrinsic, scale);
    let mut fitted = entity.clone();
    fitted.kind = CanvasEntityKind::DirectEditor { block_id, scale };
    fitted.transform.size = size;
    fitted.transform.center = CanvasPoint::new(bounds.center().x, bounds.min.y + size.y * 0.5);
    Some(fitted)
}

fn fit_scale(stage: Rect, framed: WorldRect) -> f32 {
    let size = framed.size();
    (stage.width() / size.x.max(f32::EPSILON))
        .min(stage.height() / size.y.max(f32::EPSILON))
        .max(f32::EPSILON)
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
        self.operate_all(std::slice::from_ref(operation));
    }

    fn operate_all(&self, operations: &[InfiniteCanvasOperation]) {
        if let Ok(edit) = self.cells.value().content.read(|canvas| {
            operations
                .iter()
                .flat_map(|operation| canvas.root().edit_for(operation).0)
                .collect::<Edit>()
        }) && !edit.0.is_empty()
        {
            self.cells.value().content.operate(edit);
        }
    }

    fn add_selected(&self, entities: Vec<CanvasEntity>) {
        self.selection
            .set(entities.iter().map(|entity| entity.id).collect());
        let operations = entities
            .into_iter()
            .map(|entity| InfiniteCanvasOperation::Add { entity })
            .collect::<Vec<_>>();
        self.operate_all(&operations);
    }

    pub(crate) fn record(&self, operation: InfiniteCanvasOperation) {
        self.cells.value().grouped_edit.set(false);
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
                self.cells.value().grouped_edit.set(true);
                self.operate(&operation);
            }
            false => self.record(operation),
        }
    }

    pub(crate) fn finish_grouped_edit(&self) {
        self.cells.value().grouped_edit.set(false);
    }

    pub(crate) fn default_style(&self) -> CanvasEntityStyle {
        CanvasEntityStyle {
            foreground: self.cells.value().last_foreground.get(),
            fill: self.cells.value().last_fill.get(),
            ..CanvasEntityStyle::default()
        }
    }

    pub(crate) fn remember_foreground(&self, color: CanvasColor) {
        self.cells.value().last_foreground.set(color);
    }

    pub(crate) fn remember_fill(&self, fill: Option<CanvasColor>) {
        self.cells.value().last_fill.set(fill);
    }

    pub(crate) fn set_tool(&self, tool: Tool) {
        self.tool.set(tool);
        self.gesture.set(None);
    }

    pub(crate) fn put_down_tool(&self) {
        if self.tool.get_untracked() != Tool::Select {
            self.set_tool(Tool::Select);
        }
    }

    pub(crate) fn begin_gesture(&self, gesture: Option<Gesture>) {
        self.gesture.set(gesture);
    }

    pub(crate) fn update_gesture(&self, update: impl FnOnce(&mut Gesture)) {
        let mut held = self.gesture.get_untracked();
        if let Some(gesture) = held.as_mut() {
            update(gesture);
            self.gesture.set(held);
        }
    }

    pub(crate) fn focus_editor(&self, entity: Option<Uuid>) {
        if self.focused_editor.get_untracked() != entity {
            self.focused_editor.set(entity);
        }
        if entity.is_none() {
            self.cells.value().confirmed_editor.set(None);
        }
    }

    pub(crate) fn edit_text(&self, entity: Option<Uuid>) {
        if self.editing_text.get_untracked() != entity {
            self.editing_text.set(entity);
        }
    }

    pub(crate) fn edit_text_at(&self, entity: Uuid, entry: TextEntry) {
        self.cells.value().text_entry.set(Some(entry));
        self.edit_text(Some(entity));
    }

    pub(crate) fn take_text_entry(&self) -> Option<TextEntry> {
        self.cells.value().text_entry.take()
    }

    pub(crate) fn finish_text(&self) {
        self.edit_text(None);
        self.finish_grouped_edit();
    }

    pub(crate) fn surface(&self) -> NodeRef {
        self.cells.value().surface
    }

    pub(crate) fn focus_surface(&self) {
        if let Some(surface) = self.surface().try_get() {
            with_document(|document| document.focus_focusable(surface));
        }
    }

    pub(crate) fn fit_text(&self, mut entity: CanvasEntity) -> CanvasEntity {
        let CanvasEntityKind::Text { text_style, .. } = &entity.kind else {
            return entity;
        };
        let text_style = *text_style;
        let Some(measured) = super::text::measure(&entity, self.scale()) else {
            return entity;
        };
        let mut size = text_box_size(&text_style, measured);
        if text_style.wrap {
            size = CanvasPoint::new(entity.transform.size.x, entity.transform.size.y.max(size.y));
        }
        if (entity.transform.size.x - size.x).abs() >= 0.5
            || (entity.transform.size.y - size.y).abs() >= 0.5
        {
            entity.transform = text_grown(entity.transform, &text_style, size);
        }
        entity
    }

    pub(crate) fn take_context_position(&self) -> Option<CanvasPoint> {
        self.cells.value().context_position.take()
    }

    pub(crate) fn note_context_position(&self, at: Option<CanvasPoint>) {
        self.cells.value().context_position.set(at);
    }

    pub(crate) fn note_pointer(&self, at: Option<CanvasPoint>) {
        if self.pointer.get_untracked() != at {
            self.pointer.set(at);
        }
    }

    pub(crate) fn hold_pointer(&self, down: bool) {
        self.cells.value().pointer_down.set(down);
    }

    pub(crate) fn pointer_held(&self) -> bool {
        self.cells.value().pointer_down.get()
    }

    pub(crate) fn note_touch(&self, touch: bool) {
        self.cells.value().touch.set(touch);
    }

    pub(crate) fn touching(&self) -> bool {
        self.cells.value().touch.get()
    }

    pub(crate) fn note_retap(&self, retap: Option<(Uuid, Pos2)>) {
        self.cells.value().retapped.set(retap);
    }

    pub(crate) fn take_retap(&self) -> Option<(Uuid, Pos2)> {
        self.cells.value().retapped.take()
    }

    pub(crate) fn dismiss_import_error(&self) {
        self.import_error.set(None);
    }

    pub(crate) fn request_fit_selection(&self) {
        if let Some(bounds) = self.selected_bounds() {
            self.fit_into_view(bounds);
        }
    }

    pub(crate) fn first_artboard(&self) -> Option<CanvasEntity> {
        first_artboard(&self.entities.get()).cloned()
    }

    pub(crate) fn request_fit_artboard(&self) {
        if let Some(artboard) = first_artboard(&self.entities.get_untracked()) {
            self.fit_into_view(entity_bounds(artboard));
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

    pub(crate) fn selected_line(&self) -> Option<CanvasEntity> {
        match self.selected_entities().as_slice() {
            [entity] if !entity.locked && matches!(entity.kind, CanvasEntityKind::Line) => {
                Some(entity.clone())
            }
            _ => None,
        }
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
        self.selection.set(selection);
    }

    pub(crate) fn clear_selection(&self) {
        if !self.selection.get_untracked().is_empty() {
            self.selection.set(HashSet::new());
        }
    }

    pub(crate) fn selected_bounds(&self) -> Option<WorldRect> {
        self.selected_entities()
            .iter()
            .map(entity_bounds)
            .reduce(WorldRect::union)
    }

    pub(crate) fn entity_at(&self, point: CanvasPoint) -> Option<Uuid> {
        let scale = self.scale();
        let radius = HIT_RADIUS / scale;
        let entities = self.entities.get_untracked();
        let (artboards, drawn): (Vec<_>, Vec<_>) = entities
            .iter()
            .filter(|entity| !entity.style.hidden)
            .partition(|entity| entity.is_artboard());
        drawn
            .into_iter()
            .rev()
            .find(|entity| hit_entity(entity, point, radius))
            .or_else(|| {
                artboards.into_iter().rev().find(|entity| {
                    artboard_label_hit(entity, point, scale) || hit_entity(entity, point, radius)
                })
            })
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
        self.selection.set(selection);
    }

    pub(crate) fn duplicate_selection(&self) {
        let duplicates = duplicate_entities(
            self.selected_entities(),
            CanvasPoint::new(IMPORT_CASCADE_OFFSET, IMPORT_CASCADE_OFFSET),
        );
        self.add_selected(duplicates);
    }

    pub(crate) fn begin_move(&self, world: CanvasPoint, duplicate: bool) {
        let mut originals = self.selected_unlocked();
        if duplicate {
            originals = duplicate_entities(originals, CanvasPoint::default());
            self.selection
                .set(originals.iter().map(|entity| entity.id).collect());
        }
        self.begin_gesture((!originals.is_empty()).then_some(Gesture::Move {
            start: world,
            current: world,
            originals,
            duplicate,
        }));
    }

    pub(crate) fn set_hidden(&self, id: Uuid, hidden: bool) {
        let Some(held) = self
            .entities
            .get_untracked()
            .into_iter()
            .find(|entity| entity.id == id)
        else {
            return;
        };
        let mut updated = held.clone();
        updated.style.hidden = hidden;
        self.record_update(vec![held], vec![updated], false);
    }

    pub(crate) fn move_layer(&self, moved: Uuid, onto: Uuid, in_front: bool) {
        if moved == onto {
            return;
        }
        let mut order: Vec<Uuid> = self
            .entities
            .get_untracked()
            .iter()
            .map(|entity| entity.id)
            .collect();
        order.retain(|id| *id != moved);
        let Some(at) = order.iter().position(|id| *id == onto) else {
            return;
        };
        order.insert(at + usize::from(in_front), moved);
        self.record(InfiniteCanvasOperation::ExactOrder { ids: order });
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
                self.selection.set(all);
                self.put_down_tool();
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
                self.selection.set(inverted);
                self.put_down_tool();
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
                    self.selection.set(selection);
                    self.record(InfiniteCanvasOperation::Remove { ids });
                }
            }
            CanvasCommand::Lock => self.set_selection_locked(true),
            CanvasCommand::Unlock => self.set_selection_locked(false),
            CanvasCommand::Group => self.group_selection(),
            CanvasCommand::Ungroup => self.ungroup_selection(),
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
                block_editor_beui::beui::reactive::copy_text(format!(
                    "{CANVAS_CLIPBOARD_PREFIX}{json}"
                ));
                true
            }
            Err(error) => {
                self.import_error
                    .set(Some(format!("Could not copy canvas objects: {error}")));
                false
            }
        }
    }

    fn paste_center(&self) -> CanvasPoint {
        let context = self.take_context_position();
        self.pointer
            .get_untracked()
            .or(context)
            .unwrap_or_else(|| self.viewport_center())
    }

    pub(crate) fn ask_to_paste(&self) {
        block_editor_beui::beui::reactive::request_paste();
    }

    pub(crate) fn paste_text(&self, text: &str) -> bool {
        let Some(json) = text.strip_prefix(CANVAS_CLIPBOARD_PREFIX) else {
            return false;
        };
        let payload = match serde_json::from_str::<CanvasClipboardPayload>(json) {
            Ok(payload) => payload,
            Err(error) => {
                self.import_error
                    .set(Some(format!("Could not paste canvas objects: {error}")));
                return true;
            }
        };
        let Some(bounds) = payload
            .entities
            .iter()
            .map(entity_bounds)
            .reduce(WorldRect::union)
        else {
            return true;
        };
        let from = bounds.center();
        let to = self.paste_center();
        let duplicates = duplicate_entities(
            payload.entities,
            CanvasPoint::new(to.x - from.x, to.y - from.y),
        );
        self.add_selected(duplicates);
        self.put_down_tool();
        true
    }

    pub(crate) fn open_block_picker(self, center: Option<CanvasPoint>) {
        let state = self;
        let at = center.unwrap_or_else(|| self.viewport_center());
        let on_screen = self.camera().to_screen(pos2(at.x, at.y));
        let filter = BlockFilter {
            anchor: Some(child_rect(Rect::from_min_size(on_screen, Vec2::ZERO))),
            ..BlockFilter::default()
        };
        self.editor.pick_block(filter, move |picked| {
            if let Ok(picked) = picked
                && state.is_alive()
            {
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
        self.put_down_tool();
    }

    pub(crate) fn open_image_picker(self, center: Option<CanvasPoint>) {
        self.cells.value().pending_image_center.set(center);
        let state = self;
        pick_file(image_filter(), move |pick| {
            if state.is_alive() {
                state.take_image_pick(pick);
            }
        });
    }

    pub(crate) fn open_component_picker(self, anchor: Option<ChildRect>) {
        let selected = self
            .selected_entities()
            .iter()
            .map(|entity| entity.id)
            .collect::<HashSet<_>>();
        let state = self;
        self.editor.pick_block(
            BlockFilter {
                name: "Component".to_owned(),
                block_types: vec![DatabaseSchemaContent::CONTENT_TYPE.into_bytes()],
                excluded: Vec::new(),
                templates: false,
                place: None,
                anchor,
            },
            move |picked| {
                if let Ok(picked) = picked
                    && state.is_alive()
                {
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

    pub(crate) fn open_value_picker(self, schema_id: Uuid, request: DatabaseBlockPickRequest) {
        let field_id = request.field_id;
        let selected = self.selection.get_untracked();
        let filter = match request.block_type {
            Some(block_type) => BlockFilter {
                name: "Value".to_owned(),
                block_types: vec![block_type.into_bytes()],
                excluded: Vec::new(),
                templates: false,
                place: None,
                anchor: request.anchor,
            },
            None => BlockFilter {
                anchor: request.anchor,
                ..BlockFilter::default()
            },
        };
        let state = self;
        self.editor.pick_block(filter, move |picked| {
            if let Ok(picked) = picked
                && state.is_alive()
            {
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

pub(crate) fn selected_outlines(
    entities: &[CanvasEntity],
    selection: &HashSet<Uuid>,
) -> Vec<SelectionFrame> {
    if selection.len() < 2 {
        return Vec::new();
    }
    entities
        .iter()
        .filter(|entity| selection.contains(&entity.id))
        .map(|entity| SelectionFrame {
            center: entity.transform.center,
            size: entity.transform.size,
            rotation: entity.transform.rotation,
        })
        .collect()
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
                CanvasEntityKind::Artboard { .. } | CanvasEntityKind::Text { .. } => false,
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
        let size = direct_editor_entity_size(Vec2::new(180.0, 100.0), 1.0);
        let entity_id = Uuid::new_v4();
        let transform = CanvasTransform::new(center, size, 0.0);
        let entity = CanvasEntity {
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
        };
        self.cells.value().placeholders.borrow_mut().insert(
            entity_id,
            Placeholder {
                entity: entity.clone(),
                fitted: false,
            },
        );
        self.selection.set(HashSet::from([entity_id]));
        self.record(InfiniteCanvasOperation::Add { entity });
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
                self.edit_text_at(entity.id, TextEntry::All);
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
                    let done = matches!(
                        entity.kind,
                        CanvasEntityKind::Text { .. } | CanvasEntityKind::Artboard { .. }
                    );
                    self.add_entity(entity);
                    if done {
                        self.put_down_tool();
                    }
                }
            }
            Gesture::Pen { points, pressures } => {
                if !points.is_empty() {
                    self.add_entity(pen_entity(points, pressures, self.default_style()));
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
                        .filter(|entity| {
                            !entity.style.hidden && box_rect.contains_rect(entity_bounds(entity))
                        })
                        .map(|entity| entity.id)
                        .collect::<Vec<_>>();
                    for id in hits {
                        selection.extend(Self::selection_ids(&entities, id));
                    }
                }
                self.selection.set(selection);
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
            Gesture::Endpoint { ref original, .. } => {
                let updates = preview_entities(&gesture);
                self.record_update(vec![original.clone()], updates, false);
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
            Tool::Artboard => {
                let bounds = gesture_rect(start, current, from_center);
                if bounds.size().x < MIN_SIZE || bounds.size().y < MIN_SIZE {
                    return None;
                }
                let count = self
                    .entities
                    .get_untracked()
                    .iter()
                    .filter(|entity| entity.is_artboard())
                    .count();
                Some(CanvasEntity {
                    id: Uuid::new_v4(),
                    transform: CanvasTransform::new(bounds.center(), bounds.size(), 0.0),
                    kind: CanvasEntityKind::Artboard {
                        name: format!("Artboard {}", count + 1),
                    },
                    style: CanvasEntityStyle::default(),
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
    pub(crate) fn watch(self) {
        let state = self;
        let drag = self.editor.drag();
        create_effect(move || {
            let drag = drag.get();
            untrack(|| state.take_drag(drag));
        });
        let state = self;
        let files = self.editor.files();
        create_effect(move || {
            let drop = files.get();
            untrack(|| state.take_files(drop));
        });
        if self.preview {
            return;
        }
        let state = self;
        create_effect(move || {
            state.entities.with(|_| ());
            state.child_states.with(|_| ());
            untrack(|| state.autosize_direct_editors());
        });
        let state = self;
        let visible = self.editor.presence_visible();
        create_effect(move || {
            visible.with(|_| ());
            state.pointer.with(|_| ());
            state.selection.with(|_| ());
            untrack(|| state.publish_presence());
        });
        let state = self;
        create_effect(move || {
            state.entities.with(|_| ());
            state.peers.with(|_| ());
            untrack(|| state.read_presence());
        });
        let state = self;
        let resized = self.editor.resized();
        create_effect(move || {
            let Some(size) = resized.get() else {
                return;
            };
            untrack(|| state.resize_first_artboard(size));
        });
        let state = self;
        let revealed = self.editor.revealed();
        create_effect(move || {
            let Some(client_id) = revealed.get() else {
                return;
            };
            untrack(|| state.reveal_peer(client_id));
        });
    }

    fn take_image_pick(&self, pick: FilePick) {
        match pick.map(|file| file.map(imported_image)) {
            Ok(Some(image)) => {
                self.import_error.set(None);
                let center = self
                    .cells
                    .value()
                    .pending_image_center
                    .take()
                    .unwrap_or_else(|| self.viewport_center());
                self.add_imported_image(image, center);
                self.put_down_tool();
            }
            Err(error) => {
                self.cells.value().pending_image_center.set(None);
                self.import_error.set(Some(error));
            }
            Ok(None) => self.cells.value().pending_image_center.set(None),
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
            self.cells.value().pending_file_drop.set(None);
            return;
        };
        let at = self.world_at(drop.position);
        if !drop.dropped {
            self.cells.value().pending_file_drop.set(Some(at));
            return;
        }
        self.import_error.set(None);
        let base = self.cells.value().pending_file_drop.take().unwrap_or(at);
        for (index, file) in drop.files.into_iter().enumerate() {
            let offset = IMPORT_CASCADE_OFFSET * index as f32;
            self.add_imported_image(
                ImageContent::from_file(file.name, file.data),
                CanvasPoint::new(base.x + offset, base.y + offset),
            );
        }
    }

    pub(crate) fn take_paste(&self, pasted: Pasted) {
        match pasted {
            Pasted::Text(text) => {
                self.paste_text(&text);
            }
            Pasted::Image(file) => {
                self.import_error.set(None);
                let center = self.paste_center();
                self.add_imported_image(ImageContent::from_file(file.name, file.data), center);
            }
        }
    }

    fn publish_presence(&self) {
        let cursor = self.editor.presence_visible().get_untracked().then(|| {
            let color = self.cells.value().color.get().unwrap_or_else(|| {
                let used = self
                    .peers
                    .get_untracked()
                    .into_iter()
                    .map(|(_, cursor)| cursor.color);
                let color = pick_free_color(used);
                self.cells.value().color.set(Some(color));
                color
            });
            CanvasCursor {
                pointer: self.pointer.get_untracked(),
                selection: self.selection.get_untracked().iter().copied().collect(),
                color,
            }
        });
        if *self.cells.value().shown.borrow() == cursor {
            return;
        }
        self.editor.show(cursor.as_ref());
        self.cells.value().shown.replace(cursor);
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
            self.presence.set(presence);
        }
    }

    fn resize_first_artboard(&self, size: Vec2) {
        let Some(artboard) = first_artboard(&self.entities.get_untracked()).cloned() else {
            return;
        };
        let wanted = CanvasPoint::new(size.x.max(MIN_SIZE), size.y.max(MIN_SIZE));
        let held = artboard.transform.size;
        if (wanted.x - held.x).abs() < 0.01 && (wanted.y - held.y).abs() < 0.01 {
            return;
        }
        let mut resized = artboard.clone();
        resized.transform.size = wanted;
        self.record_update(vec![artboard], vec![resized], false);
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
        let stage = self.cells.value().stage.get();
        let available = (stage.size() - Vec2::splat(40.0)).max(Vec2::splat(1.0));
        let target = bounds.rect();
        let scale = self.scale();
        let factor = (available.x / (target.width() * scale).max(1.0))
            .min(available.y / (target.height() * scale).max(1.0));
        let anchor = self.camera().to_screen(target.center());
        self.editor.zoom_at(factor, anchor);
        self.editor.pan(stage.center() - anchor);
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
            let placeholder = self
                .cells
                .value()
                .placeholders
                .borrow()
                .get(&entity.id)
                .cloned();
            if let Some(placeholder) = placeholder {
                if child.resize != ResizeMode::Both {
                    self.cells
                        .value()
                        .placeholders
                        .borrow_mut()
                        .remove(&entity.id);
                } else if placeholder.entity == entity {
                    if !placeholder.fitted
                        && let Some(fitted) = child
                            .intrinsic_size
                            .and_then(|intrinsic| fit_to_child(&entity, intrinsic))
                    {
                        if let Some(held) = self
                            .cells
                            .value()
                            .placeholders
                            .borrow_mut()
                            .get_mut(&entity.id)
                        {
                            held.fitted = true;
                        }
                        before.push(entity);
                        after.push(fitted);
                    }
                    continue;
                }
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

    fn is_placeholder(&self, entity: &CanvasEntity) -> bool {
        self.cells
            .value()
            .placeholders
            .borrow()
            .get(&entity.id)
            .is_some_and(|placeholder| placeholder.entity == *entity)
    }

    pub(crate) fn assigned_intrinsic(&self, entity: &CanvasEntity) -> Option<Vec2> {
        let CanvasEntityKind::DirectEditor { scale, .. } = entity.kind else {
            return None;
        };
        if self.peek_child(entity.id).resize != ResizeMode::Both || self.is_placeholder(entity) {
            return None;
        }
        let content = direct_editor_layout(entity)?.content.size();
        Some(Vec2::new(
            content.x / scale.max(f32::EPSILON),
            content.y / scale.max(f32::EPSILON),
        ))
    }
}
