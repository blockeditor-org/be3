use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::CanvasView;
use block_editor_beui::beui::icons::Glyph;
use database_types::DatabaseSchemaContent;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::presence::{CanvasCaret, CanvasCursor};
use block_editor_beui::ContentProjection;
use block_editor_beui::be_block::presence::{PresenceColor, pick_free_color};
use block_editor_beui::be_block::{Edit, ObjectId};
use block_editor_beui::beui::icons::{
    ICON_BRUSH, ICON_CIRCLE, ICON_DIAMOND, ICON_ERASER, ICON_LASSO_SELECT, ICON_MOUSE_POINTER_2,
    ICON_MOVE_UP_RIGHT, ICON_RECTANGLE_HORIZONTAL, ICON_SCAN, ICON_SLASH, ICON_TYPE,
};
use block_editor_beui::beui::native::with_document;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{Pos2, Rect, Thumbhash, Vec2, pos2};
use block_editor_beui::block_ui::BlockLabel;
use block_editor_beui::{
    BlockFilter, ChildState, Drag, Editor, FileDrop, InteractionMode, ResizeMode,
};
use block_editor_beui::{BlockParent, BlockQuery, PickAnchor};
use block_editor_database::DatabaseBlockPickRequest;
use block_editor_text::SharedCaret;
use database_types::database::DatabaseValue;
use infinite_canvas_types::CanvasContent;
use infinite_canvas_types::{
    Canvas, CanvasArrowhead, CanvasBinding, CanvasComponent, CanvasEntity, CanvasLayerMove,
    CanvasLook, CanvasPoint, CanvasProfile, CanvasShape, CanvasStyle, CanvasTextStyle,
    CanvasTransform, InfiniteCanvasOperation,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::*;

use super::entities::Entities;
use super::text::labels;
use crate::images::{image_filter, imported_image};
use beui::Pasted;
use image_block_types::{ImageContent, ImageFile};

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
    Lasso,
    Frame,
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    Diamond,
    Text,
    Pen,
    Eraser,
}

impl Tool {
    pub(crate) fn glyph(self) -> Glyph {
        match self {
            Tool::Select => ICON_MOUSE_POINTER_2,
            Tool::Lasso => ICON_LASSO_SELECT,
            Tool::Frame => ICON_SCAN,
            Tool::Line => ICON_SLASH,
            Tool::Arrow => ICON_MOVE_UP_RIGHT,
            Tool::Rectangle => ICON_RECTANGLE_HORIZONTAL,
            Tool::Ellipse => ICON_CIRCLE,
            Tool::Diamond => ICON_DIAMOND,
            Tool::Text => ICON_TYPE,
            Tool::Pen => ICON_BRUSH,
            Tool::Eraser => ICON_ERASER,
        }
    }

    pub(crate) fn selects(self) -> bool {
        matches!(self, Tool::Select | Tool::Lasso)
    }

    pub(crate) fn draws_a_box(self) -> bool {
        matches!(
            self,
            Tool::Rectangle | Tool::Ellipse | Tool::Diamond | Tool::Frame
        )
    }

    pub(crate) fn draws_a_line(self) -> bool {
        matches!(self, Tool::Line | Tool::Arrow)
    }

    pub(crate) fn styles(self) -> bool {
        !self.selects() && self != Tool::Eraser
    }

    pub(crate) fn belongs_to(self, profile: CanvasProfile) -> bool {
        match self {
            Tool::Frame => profile == CanvasProfile::Vector,
            _ => true,
        }
    }
}

fn seeded(mut entity: CanvasEntity, id: Uuid, start: CanvasPoint) -> CanvasEntity {
    entity.id = id;
    entity.style.seed = drawn_seed(entity.shape, start);
    entity
}

pub(crate) fn profile_style(profile: CanvasProfile) -> CanvasStyle {
    match profile {
        CanvasProfile::Prototyping => CanvasStyle {
            look: CanvasLook::HandDrawn,
            ..CanvasStyle::default()
        },
        CanvasProfile::Vector => CanvasStyle::default(),
    }
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
        id: Uuid,
        tool: Tool,
        start: CanvasPoint,
        current: CanvasPoint,
        pointer: CanvasPoint,
        from_center: bool,
        detached: bool,
    },
    Pen {
        id: Uuid,
        points: Vec<CanvasPoint>,
        pressures: Vec<f32>,
        predicted: Option<CanvasPoint>,
        ink: Option<crate::freehand::Ink>,
    },
    SelectBox {
        start: CanvasPoint,
        current: CanvasPoint,
        pointer: CanvasPoint,
        from_center: bool,
        additive: bool,
    },
    Lasso {
        points: Vec<CanvasPoint>,
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
        original: Box<CanvasEntity>,
        end: usize,
        current: CanvasPoint,
        grab: CanvasPoint,
        snap: bool,
        detached: bool,
    },
    Bend {
        original: Box<CanvasEntity>,
        index: usize,
        current: CanvasPoint,
        grab: CanvasPoint,
    },
    Rotate {
        frame: SelectionFrame,
        start_angle: f32,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        snap_angle: bool,
    },
    Erase {
        trail: Vec<CanvasPoint>,
        erased: Vec<CanvasEntity>,
    },
}

pub(crate) const ERASER_TRAIL: usize = 24;
const ERASED_OPACITY: f32 = 0.25;

pub(crate) fn gesture_entities(gesture: &Gesture) -> Vec<CanvasEntity> {
    match gesture {
        Gesture::Move {
            start,
            current,
            originals,
            ..
        } => {
            let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
            unbound_from_the_rest(
                originals,
                originals
                    .iter()
                    .cloned()
                    .map(|mut entity| {
                        entity.transform.center.x += delta.x;
                        entity.transform.center.y += delta.y;
                        entity
                    })
                    .collect(),
            )
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
        } => unbound_from_the_rest(
            originals,
            resize_entities(
                *handle,
                *frame,
                *current,
                originals,
                *preserve_aspect_ratio,
                *scale_text,
                *scale_editors,
            ),
        ),
        Gesture::Rotate {
            frame,
            start_angle,
            current,
            originals,
            snap_angle,
        } => unbound_from_the_rest(
            originals,
            rotate_entities(*frame, *start_angle, *current, originals, *snap_angle),
        ),
        Gesture::Endpoint {
            original,
            end,
            current,
            snap,
            ..
        } => {
            let at = match snap {
                true => constrain_point_angle(
                    path_neighbour(original, *end),
                    *current,
                    std::f32::consts::FRAC_PI_4,
                ),
                false => *current,
            };
            let mut moved = with_end(original.as_ref().clone(), *end, at);
            moved.bindings[*end] = None;
            vec![moved]
        }
        Gesture::Bend {
            original,
            index,
            current,
            ..
        } => vec![with_point(original.as_ref().clone(), *index, *current)],
        Gesture::Erase { erased, .. } => erased
            .iter()
            .cloned()
            .map(|mut entity| {
                entity.style.opacity *= ERASED_OPACITY;
                entity
            })
            .collect(),
        Gesture::Create { .. }
        | Gesture::Pen { .. }
        | Gesture::SelectBox { .. }
        | Gesture::Lasso { .. } => Vec::new(),
    }
}

fn unbound_from_the_rest(
    originals: &[CanvasEntity],
    mut entities: Vec<CanvasEntity>,
) -> Vec<CanvasEntity> {
    if entities.as_slice() == originals {
        return entities;
    }
    let moved: HashSet<Uuid> = entities.iter().map(|entity| entity.id).collect();
    for entity in &mut entities {
        for binding in &mut entity.bindings {
            if binding.is_some_and(|binding| !moved.contains(&binding.entity)) {
                *binding = None;
            }
        }
    }
    entities
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
    pub(crate) carets: Vec<(CanvasCaret, PresenceColor)>,
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
    pointer_down: Cell<bool>,
    touch: Cell<bool>,
    pen_seen: Cell<bool>,
    retapped: Cell<Option<(Uuid, Pos2)>>,
    text_entry: Cell<Option<TextEntry>>,
    client: u64,
    surface: NodeRef,
    select_tool: Cell<Tool>,
    children: RefCell<HashMap<Uuid, ChildState>>,
    placeholders: RefCell<HashMap<Uuid, Placeholder>>,
    confirmed_editor: Cell<Option<Uuid>>,
    shown: RefCell<Option<CanvasCursor>>,
    writing: Cell<bool>,
    scrubbing: Cell<bool>,
    written_points: Cell<usize>,
    drafting: Cell<Option<Uuid>>,
    color: Cell<Option<PresenceColor>>,
    stage: Cell<Rect>,
}

#[derive(Clone, Copy)]
pub(crate) struct CanvasState {
    cells: Stored<CanvasCells>,
    editor: Editor,
    preview: bool,
    pub(crate) pointer: Signal<Option<CanvasPoint>>,
    caret: Signal<Option<CanvasCaret>>,
    pub(crate) entities: Entities,
    pub(crate) tool: Signal<Tool>,
    pub(crate) tool_locked: Signal<bool>,
    pub(crate) binds: Signal<bool>,
    pub(crate) profile: Signal<CanvasProfile>,
    styles: Signal<HashMap<CanvasProfile, CanvasStyle>>,
    pub(crate) selection: Signal<HashSet<Uuid>>,
    pub(crate) entered_group: Signal<Option<Uuid>>,
    pub(crate) gesture: Signal<Option<Gesture>>,
    pub(crate) focused_editor: Signal<Option<Uuid>>,
    pub(crate) editing_text: Signal<Option<Uuid>>,
    pub(crate) import_error: Signal<Option<String>>,
    labels: Memo<Ready<HashMap<Uuid, BlockLabel>>>,
    types: Memo<HashMap<Uuid, Uuid>>,
    thumbhashes: Memo<HashMap<Uuid, Thumbhash>>,
    pub(crate) child_states: Signal<HashMap<Uuid, ChildState>>,
    pub(crate) presence: Signal<Presence>,
    peer_carets: Memo<Vec<(CanvasCaret, PresenceColor)>>,
    peers: ReadSignal<Vec<(u64, CanvasCursor)>>,
    own_stroke: Memo<Option<Uuid>>,
    strokes: Memo<Vec<Uuid>>,
}

impl CanvasState {
    pub(crate) fn new(editor: Editor, preview: bool) -> Self {
        let content = editor.block_content::<CanvasContent>();
        let entities = Entities::new(content);
        let tool = create_signal(Tool::Select);
        let profile = create_signal(CanvasProfile::default());
        let read_profile = create_signal(false);
        create_effect(move || {
            if read_profile.get_untracked() {
                return;
            }
            if let Ok(stored) = content.read(|canvas| canvas.root().profile) {
                read_profile.set(true);
                profile.set(stored);
            }
        });
        let styles = create_signal(HashMap::new());
        let selection = create_signal(HashSet::new());
        let entered_group = create_signal(None);
        let gesture = create_signal(None);
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
        let peer_carets = create_memo(move || presence.with(|presence| presence.carets.clone()));
        let pointer = create_signal(None);
        let caret = create_signal(None);
        let cells = store(CanvasCells {
            content,
            pending_image_center: Cell::new(None),
            pending_file_drop: Cell::new(None),
            context_position: Cell::new(None),
            pointer_down: Cell::new(false),
            touch: Cell::new(false),
            pen_seen: Cell::new(false),
            retapped: Cell::new(None),
            text_entry: Cell::new(None),
            client: Uuid::new_v4().as_u64_pair().0 | 1 << 63,
            surface: NodeRef::new(),
            select_tool: Cell::new(Tool::Select),
            children: RefCell::new(HashMap::new()),
            placeholders: RefCell::new(HashMap::new()),
            confirmed_editor: Cell::new(None),
            shown: RefCell::new(None),
            writing: Cell::new(false),
            scrubbing: Cell::new(false),
            written_points: Cell::new(0),
            drafting: Cell::new(None),
            color: Cell::new(None),
            stage: Cell::new(Rect::ZERO),
        });

        let peers = editor.peers::<CanvasCursor>();
        let own_stroke = create_memo(move || {
            gesture.with(|gesture| match gesture {
                Some(Gesture::Pen { id, .. }) => Some(*id),
                _ => None,
            })
        });
        let strokes = create_memo(move || {
            let mut strokes: Vec<Uuid> = own_stroke.get().into_iter().collect();
            peers.with(|peers| {
                strokes.extend(peers.iter().filter_map(|(_, cursor)| cursor.drawing));
            });
            strokes
        });
        Self {
            cells,
            editor,
            preview,
            pointer,
            caret,
            entities,
            tool,
            tool_locked: create_signal(false),
            binds: create_signal(true),
            profile,
            styles,
            selection,
            entered_group,
            gesture,
            focused_editor,
            editing_text,
            import_error,
            labels,
            types,
            thumbhashes,
            child_states,
            presence,
            peer_carets,
            peers,
            own_stroke,
            strokes,
        }
    }

    fn drawn_shape(&self, gesture: &Gesture) -> Option<CanvasEntity> {
        let Gesture::Create {
            id,
            tool,
            start,
            current,
            from_center,
            detached,
            ..
        } = *gesture
        else {
            return None;
        };
        let entity = self.created_entity(tool, start, current, from_center, detached)?;
        Some(seeded(entity, id, start))
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

    pub(crate) fn grid(&self) -> ReadSignal<infinite_canvas_types::CanvasGrid> {
        self.cells.value().content.field(
            block_editor_beui::be_block::be_model::ObjectId::ROOT,
            infinite_canvas_types::Canvas::GRID,
        )
    }

    pub(crate) fn set_grid(&self, grid: infinite_canvas_types::CanvasGrid) {
        self.cells
            .value()
            .content
            .operate(infinite_canvas_types::Canvas::set_grid(grid));
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
        self.preview.then(|| self.entities.region())
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

    pub(crate) fn type_value(&self, before: CanvasEntity, after: CanvasEntity) {
        self.open_writes();
        self.record_update(vec![before], vec![after]);
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
        match entity.shape {
            CanvasShape::Editor => self.peek_child(entity.id).resize,
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

const FIT_MARGIN: f32 = 20.0;

const FITTED_ROOM: Vec2 = Vec2::new(640.0, 480.0);

fn fit_to_child(entity: &CanvasEntity, intrinsic: Vec2) -> Option<CanvasEntity> {
    if entity.shape != CanvasShape::Editor {
        return None;
    }
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
    fitted.scale = scale;
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

impl CanvasState {
    fn operate(&self, operation: &InfiniteCanvasOperation) {
        self.operate_all(std::slice::from_ref(operation));
    }

    fn operate_all(&self, operations: &[InfiniteCanvasOperation]) {
        if let Ok(edit) = self.cells.value().content.read(|canvas| {
            operations
                .iter()
                .flat_map(|operation| Canvas::edit_in(canvas, operation).0)
                .collect::<Edit>()
        }) && !edit.0.is_empty()
        {
            if self.gesture.with_untracked(Option::is_some) {
                self.open_writes();
            }
            self.cells.value().content.operate(edit);
        }
    }

    fn open_writes(&self) {
        if !self.cells.value().writing.replace(true) {
            self.content().begin_gesture();
        }
    }

    pub(crate) fn scrub(&self, scrubbing: bool) {
        self.cells.value().scrubbing.set(scrubbing);
        match scrubbing {
            true => self.open_writes(),
            false => self.commit_writes(),
        }
    }

    pub(crate) fn commit_writes(&self) {
        let cells = self.cells.value();
        if cells.scrubbing.get() || cells.drafting.get().is_some() {
            return;
        }
        if cells.writing.replace(false) {
            self.content().commit_gesture();
        }
    }

    pub(crate) fn cancel_writes(&self) {
        if self.cells.value().drafting.get().is_none() {
            self.drop_writes();
        }
    }

    fn drop_writes(&self) {
        if self.cells.value().writing.replace(false) {
            self.content().cancel_gesture();
        }
    }

    fn keep_draft(&self, id: Uuid) {
        let drafting = &self.cells.value().drafting;
        if drafting.get() == Some(id) {
            drafting.set(None);
            self.commit_writes();
        }
    }

    fn holds(&self, id: Uuid) -> bool {
        self.content()
            .read(|canvas| canvas.tree().contains(ObjectId::from_uuid(id)))
            .unwrap_or(false)
    }

    fn put_live(&self, entities: Vec<CanvasEntity>) {
        let (held, added): (Vec<_>, Vec<_>) = entities
            .into_iter()
            .partition(|entity| self.holds(entity.id));
        let mut operations: Vec<InfiniteCanvasOperation> = added
            .into_iter()
            .map(|entity| InfiniteCanvasOperation::Add {
                entity: Box::new(entity),
            })
            .collect();
        if !held.is_empty() {
            operations.push(InfiniteCanvasOperation::Update { entities: held });
        }
        self.operate_all(&operations);
    }

    fn write_gesture(&self) {
        let Some(gesture) = self.gesture.get_untracked() else {
            return;
        };
        match &gesture {
            Gesture::Create {
                id,
                tool,
                start,
                current,
                ..
            } if start != current => match self.drawn_shape(&gesture) {
                Some(entity) => self.put_live(vec![entity]),
                None if self.holds(*id) => {
                    self.operate(&InfiniteCanvasOperation::Remove { ids: vec![*id] })
                }
                None => {}
            },
            Gesture::Pen {
                id,
                points,
                pressures,
                ..
            } => {
                let written = &self.cells.value().written_points;
                let from = written.get();
                if from >= 2 && self.holds(*id) {
                    if from < points.len() {
                        self.operate(&InfiniteCanvasOperation::Extend {
                            id: *id,
                            points: points[from..].to_vec(),
                            pressures: pressures[from..].to_vec(),
                        });
                    }
                } else if let [start, _, ..] = points[..] {
                    let entity =
                        pen_entity(points.clone(), pressures.clone(), self.default_style());
                    self.put_live(vec![seeded(entity, *id, start)]);
                }
                written.set(points.len());
            }
            Gesture::Move {
                duplicate: true, ..
            } => self.put_live(gesture_entities(&gesture)),
            Gesture::Endpoint { .. } => self.operate(&InfiniteCanvasOperation::Update {
                entities: self.endpoint_entities(&gesture),
            }),
            Gesture::Move { .. }
            | Gesture::Resize { .. }
            | Gesture::Rotate { .. }
            | Gesture::Bend { .. }
            | Gesture::Erase { .. } => {
                let updates = gesture_entities(&gesture);
                if !updates.is_empty() {
                    self.operate(&InfiniteCanvasOperation::Update { entities: updates });
                }
            }
            Gesture::Create { .. } | Gesture::SelectBox { .. } | Gesture::Lasso { .. } => {}
        }
    }

    fn add_selected(&self, entities: Vec<CanvasEntity>) {
        self.selection
            .set(entities.iter().map(|entity| entity.id).collect());
        let operations = entities
            .into_iter()
            .map(|entity| InfiniteCanvasOperation::Add {
                entity: Box::new(entity),
            })
            .collect::<Vec<_>>();
        self.operate_all(&operations);
    }

    pub(crate) fn record(&self, operation: InfiniteCanvasOperation) {
        self.operate(&operation);
    }

    pub(crate) fn record_update(&self, before: Vec<CanvasEntity>, after: Vec<CanvasEntity>) {
        if before == after || after.is_empty() {
            return;
        }
        self.record(InfiniteCanvasOperation::Update { entities: after });
    }

    pub(crate) fn next_style(&self) -> CanvasStyle {
        let profile = self.profile.get();
        self.styles.with(|styles| {
            styles
                .get(&profile)
                .copied()
                .unwrap_or_else(|| profile_style(profile))
        })
    }

    pub(crate) fn default_style(&self) -> CanvasStyle {
        CanvasStyle {
            start_arrowhead: CanvasArrowhead::None,
            end_arrowhead: CanvasArrowhead::None,
            seed: 0,
            ..untrack(|| self.next_style())
        }
    }

    pub(crate) fn remember_style(&self, update: impl FnOnce(&mut CanvasStyle)) {
        let profile = self.profile.get_untracked();
        self.styles.update(|styles| {
            update(
                styles
                    .entry(profile)
                    .or_insert_with(|| profile_style(profile)),
            )
        });
    }

    pub(crate) fn switch_profile(&self, profile: CanvasProfile) {
        self.profile.set(profile);
        if !self.tool.get_untracked().belongs_to(profile) {
            self.set_tool(Tool::Select);
        }
        if let Ok(edit) = self
            .cells
            .value()
            .content
            .read(|canvas| canvas.root().set_profile(profile))
            && !edit.0.is_empty()
        {
            self.cells.value().content.operate(edit);
        }
    }

    pub(crate) fn styled(&self, mut entity: CanvasEntity) -> CanvasEntity {
        entity.style = CanvasStyle {
            seed: drawn_seed(entity.shape, entity.transform.center),
            ..self.default_style()
        };
        entity
    }

    pub(crate) fn set_tool(&self, tool: Tool) {
        if tool.selects() {
            self.cells.value().select_tool.set(tool);
        }
        self.tool.set(tool);
        if self.gesture.with_untracked(Option::is_some) {
            self.begin_gesture(None);
        }
    }

    pub(crate) fn select_tool(&self) -> Tool {
        match self.tool.get() {
            tool if tool.selects() => tool,
            _ => self.cells.value().select_tool.get(),
        }
    }

    pub(crate) fn switch_select_tool(&self) {
        self.set_tool(match self.tool.get_untracked() {
            Tool::Select => Tool::Lasso,
            Tool::Lasso => Tool::Select,
            _ => self.cells.value().select_tool.get(),
        });
    }

    pub(crate) fn put_down_tool(&self) {
        if !self.tool.get_untracked().selects() {
            self.set_tool(self.cells.value().select_tool.get());
        }
    }

    pub(crate) fn begin_gesture(&self, gesture: Option<Gesture>) {
        self.cancel_writes();
        self.cells.value().written_points.set(0);
        self.gesture.set(gesture);
        self.write_gesture();
    }

    pub(crate) fn update_gesture(&self, update: impl FnOnce(&mut Gesture)) {
        let mut held = self.gesture.get_untracked();
        if let Some(gesture) = held.as_mut() {
            update(gesture);
            self.gesture.set(held);
            self.write_gesture();
        }
    }

    pub(crate) fn take_gesture(&self) -> Option<Gesture> {
        let gesture = self.gesture.get_untracked();
        if gesture.is_some() {
            self.gesture.set(None);
        }
        gesture
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
        let was = self.editing_text.get_untracked();
        if was == entity {
            return;
        }
        let content = self.content();
        match entity {
            Some(_) => {
                content.record();
                content.take_projected();
            }
            None => content.stop_recording(),
        }
        self.editing_text.set(entity);
        if let Some(was) = was {
            self.stop_editing(was);
        }
    }

    fn draft_text(&self, id: Uuid) {
        self.edit_text(Some(id));
        self.open_writes();
        self.cells.value().drafting.set(Some(id));
    }

    fn stop_editing(&self, id: Uuid) {
        let drafting = &self.cells.value().drafting;
        let draft = drafting.get() == Some(id);
        if draft {
            drafting.set(None);
        }
        let empty = self.entities.get_untracked(id).is_some_and(|entity| {
            entity.shape == CanvasShape::Text && entity.text.is_empty() && !entity.locked
        });
        if empty {
            self.selection.update(|selection| {
                selection.remove(&id);
            });
        }
        match (draft, empty) {
            (true, true) => self.drop_writes(),
            (true, false) => self.commit_writes(),
            (false, true) => self.record(InfiniteCanvasOperation::Remove { ids: vec![id] }),
            (false, false) => {}
        }
    }

    pub(crate) fn content(&self) -> ContentProjection<CanvasContent> {
        self.cells.value().content
    }

    pub(crate) fn retext(&self, id: Uuid, text: &str) {
        let content = self.content();
        let client = self.cells.value().client;
        if let Ok(Some(edit)) = content.read(|canvas| Canvas::retext(canvas, client, id, text)) {
            content.operate(edit);
        }
    }

    pub(crate) fn fit_edited_text(&self, id: Uuid, content: Vec2) -> bool {
        let Some(held) = self.entities.get_untracked(id) else {
            return false;
        };
        let fitted = fit_text(held.clone(), content);
        let grown = fitted.transform != held.transform;
        if grown {
            self.record_update(vec![held], vec![fitted]);
        }
        grown
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
    }

    pub(crate) fn surface(&self) -> NodeRef {
        self.cells.value().surface
    }

    pub(crate) fn focus_surface(&self) {
        if let Some(surface) = self.surface().try_get() {
            surface.focus();
        }
    }

    pub(crate) fn take_context_position(&self) -> Option<CanvasPoint> {
        self.cells.value().context_position.take()
    }

    pub(crate) fn note_context_position(&self, at: Option<CanvasPoint>) {
        self.cells.value().context_position.set(at);
    }

    pub(crate) fn being_drawn(&self, id: Uuid) -> bool {
        self.strokes.with(|strokes| strokes.contains(&id))
    }

    pub(crate) fn peer_carets(&self) -> Memo<Vec<(CanvasCaret, PresenceColor)>> {
        self.peer_carets
    }

    pub(crate) fn note_caret(&self, entity: Uuid, caret: Option<SharedCaret>) {
        let held = self.caret.get_untracked();
        let caret = match caret {
            Some(caret) => Some(CanvasCaret { entity, caret }),
            None if held.is_some_and(|held| held.entity != entity) => return,
            None => None,
        };
        if held != caret {
            self.caret.set(caret);
        }
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

    pub(crate) fn note_pen(&self) {
        self.cells.value().pen_seen.set(true);
    }

    pub(crate) fn pen_seen(&self) -> bool {
        self.cells.value().pen_seen.get()
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

    pub(crate) fn first_frame(&self) -> Option<CanvasEntity> {
        self.entities.first_frame()
    }

    pub(crate) fn request_fit_frame(&self) {
        if let Some(frame) = untrack(|| self.entities.first_frame()) {
            self.fit_into_view(entity_bounds(&frame));
        }
    }

    pub(crate) fn request_fit_all(&self) {
        if untrack(|| self.entities.first_frame()).is_some() {
            return self.request_fit_frame();
        }
        let extents = self
            .entities
            .all_untracked()
            .iter()
            .filter(|entity| !entity.hidden)
            .map(entity_bounds)
            .reduce(WorldRect::union);
        match extents {
            Some(extents) => self.fit_into_view_within(extents, 1.0),
            None => {
                let centre = self
                    .framed()
                    .map_or_else(CanvasPoint::default, WorldRect::center);
                self.fit_into_view_within(WorldRect::from_points(centre, centre), 1.0);
            }
        }
    }

    pub(crate) fn selected_entities(&self) -> Vec<CanvasEntity> {
        self.selection
            .with(|selection| self.entities.among(selection))
    }

    pub(crate) fn selected_unlocked(&self) -> Vec<CanvasEntity> {
        self.selected_entities()
            .into_iter()
            .filter(|entity| !entity.locked)
            .collect()
    }

    pub(crate) fn selected_path(&self) -> Option<CanvasEntity> {
        match self.selected_entities().as_slice() {
            [entity] if !entity.locked && entity.shape == CanvasShape::Path => Some(entity.clone()),
            _ => None,
        }
    }

    pub(crate) fn anchors(&self, path: &CanvasEntity) -> Vec<[CanvasPoint; 2]> {
        path.bindings
            .iter()
            .zip(path_ends(path))
            .filter_map(|(binding, end)| {
                let binding = (*binding)?;
                let target = self.entities.get(binding.entity)?;
                Some([end, target.transform.to_world(binding.anchor)])
            })
            .collect()
    }

    fn binding_for(&self, point: CanvasPoint, path: Uuid, detached: bool) -> Option<CanvasBinding> {
        match detached || !self.binds.get_untracked() {
            true => None,
            false => self.binding_at(point, path),
        }
    }

    fn binding_at(&self, point: CanvasPoint, path: Uuid) -> Option<CanvasBinding> {
        let radius = HIT_RADIUS / self.scale();
        self.entities
            .all_untracked()
            .into_iter()
            .rev()
            .filter(|entity| entity.id != path && !entity.hidden && entity.shape.bindable())
            .find(|entity| {
                let local = entity.transform.to_local(point);
                let grown = CanvasPoint::new(
                    local.x * entity.transform.size.x / (entity.transform.size.x + radius * 2.0),
                    local.y * entity.transform.size.y / (entity.transform.size.y + radius * 2.0),
                );
                entity.shape.contains_local(grown)
            })
            .map(|entity| CanvasBinding {
                entity: entity.id,
                anchor: entity.shape.within(entity.transform.to_local(point)),
            })
    }

    pub(crate) fn selection_has_unlocked(&self) -> bool {
        self.selected_entities().iter().any(|entity| !entity.locked)
    }

    fn selection_ids(&self, entities: &[CanvasEntity], id: Uuid) -> HashSet<Uuid> {
        let entered = self.entered_group.get_untracked();
        let group_id = entities
            .iter()
            .find(|entity| entity.id == id)
            .and_then(|entity| entity.group_id)
            .filter(|group| Some(*group) != entered);
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
        let entities = self.entities.all_untracked();
        let ids = self.selection_ids(&entities, id);
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

    pub(crate) fn enter_group(&self, member: Uuid) {
        let group = self
            .entities
            .get_untracked(member)
            .and_then(|entity| entity.group_id);
        if self.entered_group.get_untracked() != group {
            self.entered_group.set(group);
        }
        self.selection.set(HashSet::from([member]));
    }

    pub(crate) fn leave_group(&self) -> Option<Uuid> {
        let left = self.entered_group.get_untracked()?;
        self.entered_group.set(None);
        Some(left)
    }

    pub(crate) fn select_group(&self, group: Uuid) {
        let members = self
            .entities
            .all_untracked()
            .iter()
            .filter(|entity| entity.group_id == Some(group))
            .map(|entity| entity.id)
            .collect();
        self.selection.set(members);
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

    pub(crate) fn written_at(&self, point: CanvasPoint) -> Option<CanvasEntity> {
        let hit = self.entity_at(point);
        let entities = self.entities.all_untracked();
        let writes = |entity: &&CanvasEntity| super::text::writes(entity);
        match hit {
            Some(id) => entities
                .iter()
                .find(|entity| entity.id == id)
                .filter(writes),
            None => entities.iter().rev().filter(writes).find(|entity| {
                entity.shape != CanvasShape::Path
                    && super::text::text_box(entity).contains(pos2(point.x, point.y))
            }),
        }
        .cloned()
    }

    pub(crate) fn write_at(&self, point: CanvasPoint) {
        let text = self.text_at(point);
        let id = text.id;
        self.add_entity(text);
        self.edit_text_at(id, TextEntry::All);
    }

    fn text_at(&self, point: CanvasPoint) -> CanvasEntity {
        let mut text = self.styled(CanvasEntity::new(
            CanvasShape::Text,
            CanvasTransform::new(point, CanvasPoint::new(16.0, 32.0), 0.0),
        ));
        text.placeholder = "Text".into();
        text.transform.size.y =
            text.style.text_size.px() * text.text_style.line_height + 2.0 * TEXT_INSET;
        text.transform.center =
            CanvasPoint::new(point.x + text.transform.size.x / 2.0 - TEXT_INSET, point.y);
        text
    }

    pub(crate) fn entity_at(&self, point: CanvasPoint) -> Option<Uuid> {
        let scale = self.scale();
        let radius = HIT_RADIUS / scale;
        let entities = self.entities.all_untracked();
        let (frames, drawn): (Vec<_>, Vec<_>) = entities
            .iter()
            .filter(|entity| !entity.hidden)
            .partition(|entity| entity.is_frame());
        drawn
            .into_iter()
            .rev()
            .find(|entity| hit_entity(entity, point, radius))
            .or_else(|| {
                frames.into_iter().rev().find(|entity| {
                    frame_label_hit(entity, point, scale) || hit_entity(entity, point, radius)
                })
            })
            .map(|entity| entity.id)
    }

    pub(crate) fn begin_erasing(&self, at: CanvasPoint) {
        self.begin_gesture(Some(Gesture::Erase {
            trail: Vec::new(),
            erased: Vec::new(),
        }));
        self.erase_toward(at, false);
    }

    pub(crate) fn erase_toward(&self, at: CanvasPoint, restore: bool) {
        let Some(Gesture::Erase { trail, .. }) = self.gesture.get_untracked() else {
            return;
        };
        let from = trail.last().copied().unwrap_or(at);
        let crossed = self.erasable_between(from, at);
        let mut restored = Vec::new();
        self.update_gesture(|gesture| {
            let Gesture::Erase { trail, erased } = gesture else {
                return;
            };
            if trail.len() == ERASER_TRAIL {
                trail.remove(0);
            }
            trail.push(at);
            match restore {
                true => {
                    let (kept, back): (Vec<_>, Vec<_>) = std::mem::take(erased)
                        .into_iter()
                        .partition(|entity| !crossed.iter().any(|hit| hit.id == entity.id));
                    *erased = kept;
                    restored = back;
                }
                false => {
                    for entity in crossed {
                        if !erased.iter().any(|held| held.id == entity.id) {
                            erased.push(entity);
                        }
                    }
                }
            }
        });
        if !restored.is_empty() {
            self.operate(&InfiniteCanvasOperation::Update { entities: restored });
        }
    }

    fn erasable_between(&self, from: CanvasPoint, to: CanvasPoint) -> Vec<CanvasEntity> {
        let radius = HIT_RADIUS / self.scale();
        let steps = (distance(from, to) / radius).ceil().max(1.0) as usize;
        let samples: Vec<CanvasPoint> = (0..=steps)
            .map(|step| {
                let t = step as f32 / steps as f32;
                CanvasPoint::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
            })
            .collect();
        let swept = WorldRect::from_points(from, to).expanded(radius);
        let erasable =
            |entity: &CanvasEntity| !entity.hidden && !entity.locked && !entity.is_frame();
        let hit: Vec<CanvasEntity> = self
            .entities
            .near_untracked(swept)
            .into_iter()
            .filter_map(|id| self.entities.get_untracked(id))
            .filter(|entity| {
                erasable(entity)
                    && samples
                        .iter()
                        .any(|point| hit_entity(entity, *point, radius))
            })
            .collect();
        let entered = self.entered_group.get_untracked();
        let grouped = hit
            .iter()
            .any(|entity| entity.group_id.is_some_and(|group| Some(group) != entered));
        let crossed: HashSet<Uuid> = match grouped {
            true => {
                let entities = self.entities.all_untracked();
                hit.iter()
                    .flat_map(|entity| self.selection_ids(&entities, entity.id))
                    .collect()
            }
            false => hit.iter().map(|entity| entity.id).collect(),
        };
        untrack(|| self.entities.among(&crossed))
            .into_iter()
            .filter(erasable)
            .collect()
    }

    pub(crate) fn add_entity(&self, entity: CanvasEntity) {
        let (id, shape) = (entity.id, entity.shape);
        if shape == CanvasShape::Text {
            self.draft_text(id);
        }
        self.record(InfiniteCanvasOperation::Add {
            entity: Box::new(entity),
        });
        self.select_added(id, shape);
    }

    fn select_added(&self, id: Uuid, shape: CanvasShape) {
        let select_after_add = !matches!(
            shape,
            CanvasShape::Path
                | CanvasShape::Rectangle
                | CanvasShape::Ellipse
                | CanvasShape::Diamond
                | CanvasShape::Freehand
        );
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
            .all_untracked()
            .into_iter()
            .find(|entity| entity.id == id)
        else {
            return;
        };
        let mut updated = held.clone();
        updated.hidden = hidden;
        self.record_update(vec![held], vec![updated]);
    }

    pub(crate) fn move_layer(&self, moved: Uuid, onto: Uuid, in_front: bool) {
        if moved == onto {
            return;
        }
        let mut order: Vec<Uuid> = self
            .entities
            .all_untracked()
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
        self.record_update(before, after);
    }

    pub(crate) fn ungroup_selection(&self) {
        let before = self.selected_entities();
        let groups: HashSet<_> = before.iter().filter_map(|entity| entity.group_id).collect();
        if groups.is_empty() {
            return;
        }
        let before = self
            .entities
            .all_untracked()
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
        self.record_update(before, after);
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
        self.record_update(before, after);
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
        self.record_update(before, after);
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
        self.record_update(ordered, after);
    }

    pub(crate) fn can_reorder(&self, movement: CanvasLayerMove) -> bool {
        let selected: HashSet<_> = self
            .selected_entities()
            .iter()
            .filter(|entity| !entity.locked)
            .map(|entity| entity.id)
            .collect();
        let entities = self.entities.ids().get();
        if selected.is_empty() {
            return false;
        }
        match movement {
            CanvasLayerMove::BringToFront | CanvasLayerMove::ForwardOne => {
                entities.iter().enumerate().any(|(index, id)| {
                    selected.contains(id)
                        && entities[index + 1..]
                            .iter()
                            .any(|later| !selected.contains(later))
                })
            }
            CanvasLayerMove::BackOne | CanvasLayerMove::SendToBack => {
                entities.iter().enumerate().any(|(index, id)| {
                    selected.contains(id)
                        && entities[..index]
                            .iter()
                            .any(|earlier| !selected.contains(earlier))
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
                    .all_untracked()
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
                    .all_untracked()
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
        let at = center.unwrap_or_else(|| self.viewport_center());
        let on_screen = self.camera().to_screen(pos2(at.x, at.y));
        self.pick_block_from(center, Rect::from_min_size(on_screen, Vec2::ZERO));
    }

    pub(crate) fn open_block_picker_at(self, button: NodeRef) {
        self.pick_block_from(None, button);
    }

    fn pick_block_from(self, center: Option<CanvasPoint>, anchor: impl Into<PickAnchor>) {
        let state = self;
        self.editor
            .pick_block(BlockFilter::default(), anchor, move |picked| {
                if let Ok(picked) = picked
                    && state.is_alive()
                {
                    state.place_picked_block(picked.id(), center);
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

    pub(crate) fn open_component_picker(self, anchor: Option<Rect>) {
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
            },
            anchor,
            move |picked| {
                if let Ok(picked) = picked
                    && state.is_alive()
                {
                    state.attach_picked_component(&selected, picked.id());
                }
            },
        );
    }

    fn attach_picked_component(&self, selected: &HashSet<Uuid>, reference: Uuid) {
        let before = self.entities_among(selected);
        let mut after = before.clone();
        attach_component(&mut after, selected, reference);
        self.record_update(before, after);
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
            },
            None => BlockFilter::default(),
        };
        let state = self;
        self.editor
            .pick_block(filter, request.anchor, move |picked| {
                if let Ok(picked) = picked
                    && state.is_alive()
                {
                    state.set_picked_value(&selected, schema_id, field_id, picked.id());
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
        self.record_update(before, after);
    }

    fn entities_among(&self, selected: &HashSet<Uuid>) -> Vec<CanvasEntity> {
        untrack(|| self.entities.among(selected))
    }

    pub(crate) fn update_selected(
        &self,
        compatible: impl Fn(&CanvasEntity) -> bool,
        mut update: impl FnMut(&mut CanvasStyle),
    ) {
        self.remember_style(&mut update);
        let before = self
            .selected_entities()
            .into_iter()
            .filter(|entity| compatible(entity))
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                update(&mut entity.style);
                entity
            })
            .collect();
        self.record_update(before, after);
    }

    pub(crate) fn update_selected_text(
        &self,
        mut update: impl FnMut(&mut CanvasTextStyle, &mut CanvasStyle),
    ) {
        let before = self
            .selected_entities()
            .into_iter()
            .filter(|entity| entity.shape == CanvasShape::Text)
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                update(&mut entity.text_style, &mut entity.style);
                entity
            })
            .collect::<Vec<_>>();
        if let Some(text_size) = after.first().map(|entity| entity.style.text_size) {
            self.remember_style(|style| style.text_size = text_size);
        }
        self.record_update(before, after);
    }

    pub(crate) fn replace_referenced_block(&self, old: Uuid, new: Uuid) -> bool {
        let replaced = self
            .entities
            .all_untracked()
            .into_iter()
            .filter(|entity| entity.shape.embeds_block() && entity.block == Some(old))
            .map(|entity| CanvasEntity {
                block: Some(new),
                ..entity
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
        self.record_update(before, after);
    }

    pub(crate) fn selection_defaults_to_proportional(&self) -> bool {
        self.selected_unlocked().iter().any(|entity| {
            entity.shape.embeds_block()
                && self
                    .peek_child(entity.id)
                    .capabilities
                    .preserve_aspect_ratio
        })
    }

    pub(crate) fn selection_forces_proportional(&self) -> bool {
        self.selected_unlocked().iter().any(|entity| {
            entity.shape == CanvasShape::Editor
                && self
                    .peek_child(entity.id)
                    .capabilities
                    .preserve_aspect_ratio
        })
    }

    pub(crate) fn selection_allows_rotation(&self) -> bool {
        self.selected_unlocked()
            .iter()
            .all(|entity| match entity.shape {
                CanvasShape::Editor => {
                    let state = self.peek_child(entity.id);
                    !state.placed || state.capabilities.rotation
                }
                CanvasShape::Frame | CanvasShape::Text => false,
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
        let entity = CanvasEntity {
            block: Some(block_id),
            ..CanvasEntity::new(CanvasShape::Editor, CanvasTransform::new(center, size, 0.0))
        };
        let entity_id = entity.id;
        self.cells.value().placeholders.borrow_mut().insert(
            entity_id,
            Placeholder {
                entity: entity.clone(),
                fitted: false,
            },
        );
        self.selection.set(HashSet::from([entity_id]));
        self.record(InfiniteCanvasOperation::Add {
            entity: Box::new(entity),
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
        let Some(block_id) = entity.block else {
            return;
        };
        let updated = match entity.shape {
            CanvasShape::Editor => {
                if self.focused_editor.get_untracked() == Some(entity.id) {
                    self.focus_editor(None);
                }
                direct_editor_to_preview(entity, block_id)
            }
            CanvasShape::Block => self
                .peek_child(entity.id)
                .intrinsic_size
                .map(|intrinsic| preview_to_direct_editor(entity, block_id, intrinsic)),
            _ => None,
        };
        if let Some(updated) = updated {
            self.record_update(vec![entity.clone()], vec![updated]);
        }
    }

    pub(crate) fn edit_selected(&self) {
        let selected = self.selected_entities();
        let [entity] = selected.as_slice() else {
            return;
        };
        match (entity.shape, entity.block) {
            (CanvasShape::Editor, Some(block_id)) => {
                if self.interaction(entity.id) == Some(InteractionMode::Playback) {
                    self.open_referenced_block(block_id);
                } else {
                    self.focus_editor(Some(entity.id));
                }
            }
            (shape, _) if !entity.locked && (shape == CanvasShape::Text || labels(shape)) => {
                self.edit_text_at(entity.id, TextEntry::All);
            }
            (CanvasShape::Block | CanvasShape::Image, Some(block_id)) => {
                self.open_referenced_block(block_id)
            }
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
            Gesture::Create { .. } => {
                let Some(entity) = self.drawn_shape(&gesture) else {
                    return;
                };
                let (created, shape) = (entity.id, entity.shape);
                if shape == CanvasShape::Text {
                    self.draft_text(created);
                }
                self.put_live(vec![entity]);
                self.select_added(created, shape);
                if !self.tool_locked.get_untracked() {
                    self.put_down_tool();
                    self.selection.set(HashSet::from([created]));
                }
            }
            Gesture::Pen {
                id,
                points,
                pressures,
                ..
            } => {
                if let Some(&start) = points.first() {
                    let entity = pen_entity(points, pressures, self.default_style());
                    self.put_live(vec![seeded(entity, id, start)]);
                    self.select_added(id, CanvasShape::Freehand);
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
                    let entities = self.entities.all_untracked();
                    let box_rect = gesture_rect(start, current, from_center);
                    let hits = entities
                        .iter()
                        .filter(|entity| {
                            !entity.hidden && box_rect.contains_rect(entity_bounds(entity))
                        })
                        .map(|entity| entity.id)
                        .collect::<Vec<_>>();
                    for id in hits {
                        selection.extend(self.selection_ids(&entities, id));
                    }
                }
                self.selection.set(selection);
            }
            Gesture::Lasso { points, additive } => {
                let mut selection = match additive {
                    true => self.selection.get_untracked(),
                    false => HashSet::new(),
                };
                let entities = self.entities.all_untracked();
                let hits = entities
                    .iter()
                    .filter(|entity| !entity.hidden && lasso_encloses(&points, entity))
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>();
                for id in hits {
                    selection.extend(self.selection_ids(&entities, id));
                }
                self.selection.set(selection);
            }
            Gesture::Erase { erased, .. } => {
                let ids: Vec<Uuid> = erased.iter().map(|entity| entity.id).collect();
                if !ids.is_empty() {
                    let mut selection = self.selection.get_untracked();
                    selection.retain(|id| !ids.contains(id));
                    self.selection.set(selection);
                    self.record(InfiniteCanvasOperation::Remove { ids });
                }
            }
            Gesture::Endpoint { .. } => self.record(InfiniteCanvasOperation::Update {
                entities: self.endpoint_entities(&gesture),
            }),
            Gesture::Move { .. }
            | Gesture::Resize { .. }
            | Gesture::Rotate { .. }
            | Gesture::Bend { .. } => {}
        }
    }

    fn endpoint_entities(&self, gesture: &Gesture) -> Vec<CanvasEntity> {
        let Gesture::Endpoint { end, detached, .. } = *gesture else {
            return Vec::new();
        };
        let mut moved = gesture_entities(gesture);
        for path in &mut moved {
            let at = path_ends(path)[end];
            path.bindings[end] = self.binding_for(at, path.id, detached);
        }
        moved
    }

    fn created_entity(
        &self,
        tool: Tool,
        start: CanvasPoint,
        current: CanvasPoint,
        from_center: bool,
        detached: bool,
    ) -> Option<CanvasEntity> {
        let boxed = |shape: CanvasShape| {
            let bounds = gesture_rect(start, current, from_center);
            self.styled(CanvasEntity::new(
                shape,
                CanvasTransform::new(
                    bounds.center(),
                    CanvasPoint::new(bounds.size().x.max(MIN_SIZE), bounds.size().y.max(MIN_SIZE)),
                    0.0,
                ),
            ))
        };
        match tool {
            Tool::Line | Tool::Arrow if distance(start, current) >= MIN_SIZE => {
                let mut path = self.styled(CanvasEntity::path(&[start, current]));
                if tool == Tool::Arrow {
                    path.style.end_arrowhead = CanvasArrowhead::Arrow;
                }
                path.bindings = [
                    self.binding_for(start, path.id, detached),
                    self.binding_for(current, path.id, detached),
                ];
                Some(path)
            }
            Tool::Rectangle => Some(boxed(CanvasShape::Rectangle)),
            Tool::Ellipse => Some(boxed(CanvasShape::Ellipse)),
            Tool::Diamond => Some(boxed(CanvasShape::Diamond)),
            Tool::Text if distance(start, current) < MIN_SIZE => Some(self.text_at(start)),
            Tool::Text => {
                let bounds = WorldRect::from_points(start, current);
                let size = CanvasPoint::new(bounds.size().x.max(60.0), bounds.size().y.max(32.0));
                let mut text = self.styled(CanvasEntity::new(
                    CanvasShape::Text,
                    CanvasTransform::new(bounds.center(), size, 0.0),
                ));
                text.placeholder = "Text".into();
                text.text_style = CanvasTextStyle {
                    wrap: true,
                    ..CanvasTextStyle::default()
                };
                Some(text)
            }
            Tool::Frame => {
                let bounds = gesture_rect(start, current, from_center);
                if bounds.size().x < MIN_SIZE || bounds.size().y < MIN_SIZE {
                    return None;
                }
                let count = self
                    .entities
                    .all_untracked()
                    .iter()
                    .filter(|entity| entity.is_frame())
                    .count();
                let mut frame = CanvasEntity::new(
                    CanvasShape::Frame,
                    CanvasTransform::new(bounds.center(), bounds.size(), 0.0),
                );
                frame.text = format!("Frame {}", count + 1).as_str().into();
                Some(frame)
            }
            Tool::Line | Tool::Arrow | Tool::Select | Tool::Lasso | Tool::Pen | Tool::Eraser => {
                None
            }
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
            let Some(id) = state.editing_text.get() else {
                return;
            };
            if state
                .entities
                .get(id)
                .is_some_and(|text| !text.text.is_empty())
            {
                untrack(|| {
                    with_document(|document| {
                        document.report_after_layout(move || {
                            if state.is_alive() {
                                state.keep_draft(id);
                            }
                        })
                    })
                });
            }
        });
        let state = self;
        create_effect(move || {
            state.entities.track_edits();
            state.child_states.with(|_| ());
            untrack(|| state.autosize_direct_editors());
        });
        let state = self;
        let visible = self.editor.presence_visible();
        create_effect(move || {
            visible.with(|_| ());
            state.pointer.with(|_| ());
            state.caret.with(|_| ());
            state.selection.with(|_| ());
            state.own_stroke.with(|_| ());
            untrack(|| state.publish_presence());
        });
        let state = self;
        create_effect(move || {
            state.entities.track_edits();
            state.peers.with(|_| ());
            untrack(|| state.read_presence());
        });
        let state = self;
        let resized = self.editor.resized();
        create_effect(move || {
            let Some(size) = resized.get() else {
                return;
            };
            untrack(|| state.resize_first_frame(size));
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
                ImageFile::from_file(file.name, file.data),
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
                self.add_imported_image(ImageFile::from_file(file.name, file.data), center);
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
                caret: self.caret.get_untracked(),
                drawing: self.own_stroke.get_untracked(),
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
        let mut presence = Presence::default();
        for (_, cursor) in self.peers.get_untracked() {
            let color = cursor.color;
            if !cursor.selection.is_empty()
                && let Some(bounds) = cursor
                    .selection
                    .iter()
                    .filter_map(|id| self.entities.get_untracked(*id))
                    .map(|entity| entity_bounds(&entity))
                    .reduce(WorldRect::union)
            {
                presence.selections.push(RemoteSelection {
                    frame: SelectionFrame::from_world_rect(bounds),
                    color,
                });
            }
            if let Some(caret) = cursor.caret {
                presence.carets.push((caret, color));
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

    fn resize_first_frame(&self, size: Vec2) {
        let Some(frame) = untrack(|| self.entities.first_frame()) else {
            return;
        };
        let wanted = CanvasPoint::new(size.x.max(MIN_SIZE), size.y.max(MIN_SIZE));
        let held = frame.transform.size;
        if (wanted.x - held.x).abs() < 0.01 && (wanted.y - held.y).abs() < 0.01 {
            return;
        }
        let mut resized = frame.clone();
        resized.transform.size = wanted;
        self.record_update(vec![frame], vec![resized]);
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
        let entities = self.entities.all_untracked();
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
        self.fit_into_view_within(bounds, f32::INFINITY);
    }

    fn fit_into_view_within(&self, bounds: WorldRect, max_scale: f32) {
        let Some(view) = self.editor.canvas().get_untracked() else {
            return;
        };
        let shown = self.camera().rect_to_screen(bounds.rect());
        let stage = self.cells.value().stage.get();
        self.editor.fit(
            view.rect_to_canvas(shown),
            stage.expand(-FIT_MARGIN),
            max_scale,
        );
    }

    fn autosize_direct_editors(&self) {
        let mut before = Vec::new();
        let mut after = Vec::new();
        let placed: Vec<Uuid> = self
            .cells
            .value()
            .children
            .borrow()
            .iter()
            .filter(|(_, child)| child.placed)
            .map(|(id, _)| *id)
            .collect();
        for entity in placed
            .into_iter()
            .filter_map(|id| self.entities.get_untracked(id))
        {
            if entity.shape != CanvasShape::Editor {
                continue;
            }
            let scale = entity.scale;
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
        self.record_update(before, after);
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
        if entity.shape != CanvasShape::Editor {
            return None;
        }
        let scale = entity.scale;
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

fn fit_text(mut entity: CanvasEntity, content: Vec2) -> CanvasEntity {
    if entity.shape != CanvasShape::Text {
        return entity;
    }
    let text_style = entity.text_style;
    let mut size = text_box_size(&text_style, entity.style.text_size.px(), content);
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
