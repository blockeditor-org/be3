mod app_impl;
mod canvas;
mod challenge;
mod dynamic_artifact;
mod geometry;
mod glyph;
mod hotbar;
mod hotbar_ui;
mod panels;
mod render;
mod session;
mod simulation;
mod ui;

pub use app_impl::LogicGridApp;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    rc::Rc,
};

use beui::reactive::CanvasView;
use beui::{Color32, Key, Pos2, Rect, TextAlign, Vec2};
use block::Block;
use block_client::root_settings::RootSetting;
use block_client::{
    BlockClient,
    blocks::{
        compiled_logic::CompiledLogic as CompiledBlock, hotbar::Hotbar, logic_grid::LogicGrid,
    },
};
use block_editor_plugin::ContentProjection;
use block_editor_plugin::be_block::compiled_logic::CompiledLogic;
use block_editor_plugin::be_block::logic_grid::LogicGridOperation;
use block_editor_plugin::be_block::{
    CompiledLogicContent, CompiledLogicDocument, LogicGridContent, ObjectId,
};
use logicgame::{
    challenges::{Challenge, ChallengeId, generate_challenge},
    execution::{Component as ExecutionComponent, Instruction, Pc, Vm},
    grid::{
        CircuitGraph, Component, ComponentId, ComponentKind, ComponentOrientation, ComponentSide,
        ConnectionSlot, GraphNode, GraphNodeId, InputId, LogicGrid as Grid, OutputId, Point,
        Rotation, Scale, ValidationError, Wire, value_mask,
    },
};
use uuid::Uuid;

use crate::frame::{
    DrawRay, DrawStub, DrawTriangle, DrawValueTriangle, DrawWire, RenderFrame, WireValue,
};
use geometry::*;
use hotbar::*;
use render::*;
use simulation::*;

#[cfg(test)]
use challenge::*;

const CELL: f32 = 24.0;
const WIRE_HIT_RADIUS: f32 = 7.0;
const SCALES: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];

const LABEL_COLOR: Color32 = Color32::from_rgb(232, 236, 245);

const NAME_COLOR: Color32 = Color32::from_rgb(232, 236, 245);

const PORT_LABEL_COLOR: Color32 = Color32::from_rgb(176, 188, 208);
const GRAPH_NODE_SIZE: Vec2 = Vec2::new(150.0, 48.0);
const GRAPH_COLUMN_GAP: f32 = 70.0;
const GRAPH_ROW_GAP: f32 = 18.0;
const GRAPH_MARGIN: f32 = 24.0;
const HOTBAR_KEYS: [Key; 10] = [
    Key::One,
    Key::Two,
    Key::Three,
    Key::Four,
    Key::Five,
    Key::Six,
    Key::Seven,
    Key::Eight,
    Key::Nine,
    Key::Zero,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ToolKind {
    Select,
    Wire,
    Not,
    MergerSplitter,
    Led,
    Storage,
    Input,
    Output,
    ConfigureStorage,

    Custom,
}

impl ToolKind {
    fn id(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Wire => "wire",
            Self::Not => "not",
            Self::MergerSplitter => "merger_splitter",
            Self::Led => "led",
            Self::Storage => "storage",
            Self::Input => "input",
            Self::Output => "output",
            Self::ConfigureStorage => "configure_storage",
            Self::Custom => "custom",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "select" => Self::Select,
            "wire" => Self::Wire,
            "not" => Self::Not,
            "merger_splitter" => Self::MergerSplitter,
            "led" => Self::Led,
            "storage" => Self::Storage,
            "input" => Self::Input,
            "output" => Self::Output,
            "configure_storage" => Self::ConfigureStorage,
            "custom" => Self::Custom,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Wire => "Wire",
            Self::Not => "NOT gate",
            Self::MergerSplitter => "Merger/Splitter",
            Self::Led => "LED",
            Self::Storage => "Storage",
            Self::Input => "Input",
            Self::Output => "Output",
            Self::ConfigureStorage => "Configure storage",
            Self::Custom => "Component",
        }
    }

    fn places_component(self) -> bool {
        matches!(
            self,
            Self::Not
                | Self::MergerSplitter
                | Self::Led
                | Self::Storage
                | Self::Input
                | Self::Output
                | Self::Custom
        )
    }
}

#[derive(Clone, Debug)]
enum HotbarSlot {
    Builtin(ToolKind),
    Locked {
        name: String,
    },
    Folder {
        name: String,
        slots: Vec<HotbarSlot>,
    },
    Component {
        name: String,
        compiled: Uuid,

        kind: Option<ComponentKind>,
    },
}

impl HotbarSlot {
    fn label(&self) -> &str {
        match self {
            Self::Builtin(kind) => kind.label(),
            Self::Locked { name } | Self::Folder { name, .. } | Self::Component { name, .. } => {
                name
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tool {
    kind: ToolKind,
    scale: Scale,
    merger_out_scale: Scale,
}

impl Tool {
    fn conversion_scales(self) -> (Scale, Scale) {
        match self.kind {
            ToolKind::MergerSplitter => (self.scale, self.merger_out_scale),
            _ => (self.scale, self.scale),
        }
    }

    fn snap(self) -> Scale {
        match self.kind {
            ToolKind::MergerSplitter => self.scale.max(self.merger_out_scale),
            _ => self.scale,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Camera {
    center: [f32; 2],
    zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            zoom: CELL,
        }
    }
}

impl Camera {
    fn from_view(view: Option<CanvasView>, world: Option<Vec2>, rect: Rect) -> Self {
        let view = view.unwrap_or_else(|| CanvasView::new(rect.min, 1.0));
        let scale = view.scale.max(f32::EPSILON);
        let world = world.unwrap_or_else(|| Vec2::new(rect.width() / scale, rect.height() / scale));
        let center = view.to_canvas(rect.center());
        Self {
            center: [
                (center.x - world.x * 0.5) / CELL,
                (center.y - world.y * 0.5) / CELL,
            ],
            zoom: CELL * scale,
        }
    }

    fn view(self, rect: Rect) -> CanvasView {
        CanvasView::new(
            Pos2::new(
                rect.center().x - self.center[0] * self.zoom,
                rect.center().y - self.center[1] * self.zoom,
            ),
            self.zoom,
        )
    }

    fn screen_to_world(self, screen: Pos2, rect: Rect) -> [f32; 2] {
        let world = self.view(rect).to_canvas(screen);
        [world.x, world.y]
    }

    #[cfg(test)]
    fn world_to_screen(self, world: [f32; 2], rect: Rect) -> Pos2 {
        self.view(rect).to_screen(Pos2::new(world[0], world[1]))
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Gesture {
    Wire {
        start: Point,
    },
    Not {
        anchor: Point,
        drag_start: [f32; 2],
    },
    MergerSplitter {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Led {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Storage {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Input {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Output {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Subcomponent {
        anchor: Point,
        drag_start: [f32; 2],
        kind: ComponentKind,
    },
    SelectBox {
        start: [f32; 2],
        additive: bool,
    },
    MoveSelection {
        start: [f32; 2],
        scale: Scale,
        components: Vec<(ComponentId, Point)>,
        wires: Vec<SelectedWire>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DebugEntity {
    Component(ComponentId),
    Wire(Wire),
    WireEndpoint(WireEndpoint),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum WireEnd {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct WireEndpoint {
    wire: Wire,
    end: WireEnd,
}

impl WireEndpoint {
    fn point(self) -> Point {
        match self.end {
            WireEnd::Start => self.wire.start,
            WireEnd::End => self.wire.end,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedWire {
    wire: Wire,
    start: bool,
    end: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Selection {
    components: BTreeSet<ComponentId>,
    wire_endpoints: BTreeSet<WireEndpoint>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct GraphHover {
    components: BTreeSet<ComponentId>,
    connections: BTreeSet<(ComponentId, ConnectionSlot)>,
    wires: BTreeSet<Wire>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SimulationSnapshot {
    components: Vec<Component>,
    wires: Vec<Wire>,
    graph: CircuitGraph,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Simulation {
    snapshot: Option<SimulationSnapshot>,
    vm: Option<Vm>,
    error: Option<String>,
    input_values: Vec<u64>,
    steps: u64,
    instruction_selection: SimulationInstructionSelection,
    tick_in_progress: bool,
}

#[derive(Debug)]
struct ChallengeState {
    id: ChallengeId,
    data: Challenge,
    test: ChallengeTest,

    passed_event: bool,
}

#[derive(Debug, Default)]
struct ChallengeTest {
    snapshot: Option<SimulationSnapshot>,
    error: Option<String>,

    input_slots: Vec<Option<usize>>,

    output_slots: Vec<Option<usize>>,

    next_tick: usize,

    actual: Vec<Vec<u64>>,

    mismatched: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum SimulationInstructionSelection {
    #[default]
    Active,
    ReturnFrame(usize),
    Component(Rc<ExecutionComponent>),
}

impl GraphHover {
    fn include_node(&mut self, node: &GraphNode) {
        match node {
            GraphNode::Component { component } => {
                self.components.insert(*component);
            }
            GraphNode::WireNet { wires } => {
                self.wires.extend(wires.iter().copied());
            }
            GraphNode::Connection {
                component,
                slot,
                direction,
                side,
                start,
                end,
                scale,
            } => {
                self.connections.insert((
                    *component,
                    ConnectionSlot {
                        id: *slot,
                        direction: *direction,
                        side: *side,
                        start: *start,
                        end: *end,
                        scale: *scale,
                    },
                ));
            }
        }
    }
}

impl Selection {
    fn is_empty(&self) -> bool {
        self.components.is_empty() && self.wire_endpoints.is_empty()
    }

    fn clear(&mut self) {
        self.components.clear();
        self.wire_endpoints.clear();
    }

    fn contains(&self, entity: DebugEntity) -> bool {
        match entity {
            DebugEntity::Component(id) => self.components.contains(&id),
            DebugEntity::Wire(wire) => self
                .wire_endpoints
                .iter()
                .any(|endpoint| endpoint.wire == wire),
            DebugEntity::WireEndpoint(endpoint) => self.wire_endpoints.contains(&endpoint),
        }
    }

    fn insert(&mut self, entity: DebugEntity) {
        match entity {
            DebugEntity::Component(id) => {
                self.components.insert(id);
            }
            DebugEntity::Wire(wire) => {
                self.wire_endpoints.extend([
                    WireEndpoint {
                        wire,
                        end: WireEnd::Start,
                    },
                    WireEndpoint {
                        wire,
                        end: WireEnd::End,
                    },
                ]);
            }
            DebugEntity::WireEndpoint(endpoint) => {
                self.wire_endpoints.insert(endpoint);
            }
        }
    }

    fn toggle(&mut self, entity: DebugEntity) {
        match entity {
            DebugEntity::Component(id) => {
                if !self.components.remove(&id) {
                    self.components.insert(id);
                }
            }
            DebugEntity::Wire(wire) => {
                let endpoints = [
                    WireEndpoint {
                        wire,
                        end: WireEnd::Start,
                    },
                    WireEndpoint {
                        wire,
                        end: WireEnd::End,
                    },
                ];
                if endpoints
                    .iter()
                    .any(|endpoint| self.wire_endpoints.contains(endpoint))
                {
                    for endpoint in endpoints {
                        self.wire_endpoints.remove(&endpoint);
                    }
                } else {
                    self.wire_endpoints.extend(endpoints);
                }
            }
            DebugEntity::WireEndpoint(endpoint) => {
                if !self.wire_endpoints.remove(&endpoint) {
                    self.wire_endpoints.insert(endpoint);
                }
            }
        }
    }

    fn selected_wires(&self) -> Vec<SelectedWire> {
        let mut wires = BTreeMap::<Wire, SelectedWire>::new();
        for endpoint in &self.wire_endpoints {
            let selected = wires.entry(endpoint.wire).or_insert(SelectedWire {
                wire: endpoint.wire,
                start: false,
                end: false,
            });
            match endpoint.end {
                WireEnd::Start => selected.start = true,
                WireEnd::End => selected.end = true,
            }
        }
        wires.into_values().collect()
    }
}

pub(super) enum GridStore {
    Live {
        editor: block_editor_plugin::Editor,
        content: Rc<ContentProjection<LogicGridContent>>,
    },
    #[cfg(test)]
    Local {
        content: LogicGridContent,
        revision: u64,
    },
}

impl GridStore {
    fn revision(&self) -> Option<u64> {
        match self {
            Self::Live { content, .. } => content.revision(),
            #[cfg(test)]
            Self::Local { revision, .. } => Some(*revision),
        }
    }

    fn read<T>(&self, read: impl FnOnce(&LogicGridContent) -> T) -> Option<T> {
        match self {
            Self::Live { content, .. } => content.read(read),
            #[cfg(test)]
            Self::Local { content, .. } => Some(read(content)),
        }
    }

    fn operate(&mut self, operations: &[LogicGridOperation]) {
        let Some(edit) = self.read(|content| content.root().edit_for_all(operations)) else {
            return;
        };
        if edit.0.is_empty() {
            return;
        }
        match self {
            Self::Live { content, .. } => content.operate(edit),
            #[cfg(test)]
            Self::Local { content, revision } => {
                content.apply(&edit);
                *revision += 1;
            }
        }
    }

    fn editor(&self) -> Option<&block_editor_plugin::Editor> {
        match self {
            Self::Live { editor, .. } => Some(editor),
            #[cfg(test)]
            Self::Local { .. } => None,
        }
    }
}

struct CompiledSource {
    content: Rc<ContentProjection<CompiledLogicContent>>,
    revision: Option<u64>,
    program: Option<CompiledLogic>,
}

pub(super) struct LogicGridEditor {
    store: GridStore,

    grid: Grid,
    observed_revision: Option<u64>,

    hotbar_block: Option<RootSetting<Hotbar, block_editor_plugin::be_block::HotbarContent>>,

    hotbar_needs_write: bool,

    compiled: HashMap<Uuid, CompiledSource>,
    tool: Tool,
    placement_orientation: ComponentOrientation,
    camera: Camera,
    gesture: Option<Gesture>,
    selection: Selection,
    configured_storage: Option<ComponentId>,
    simulation: Simulation,
    challenge: Option<ChallengeState>,

    hotbar: Vec<HotbarSlot>,

    active_hotbar_folder: Vec<usize>,

    active_hotbar_slot: Option<Vec<usize>>,

    hotbar_drag: Option<Vec<usize>>,
    confirm_hotbar_reset: bool,

    io_label: String,
    compile_error: Option<String>,
}

const DISPLAY_NAME: &str = "Logic Grid";

impl LogicGridEditor {
    fn live(editor: &block_editor_plugin::Editor) -> Self {
        Self::new(GridStore::Live {
            editor: editor.clone(),
            content: editor.block_content::<LogicGridContent>(),
        })
    }

    fn new(store: GridStore) -> Self {
        Self {
            store,
            grid: Grid::new(),
            observed_revision: None,
            hotbar_block: None,
            hotbar_needs_write: false,
            compiled: HashMap::new(),
            tool: Tool {
                kind: ToolKind::Select,
                scale: Scale::ONE,
                merger_out_scale: Scale::new(4).expect("default scale is valid"),
            },
            placement_orientation: ComponentOrientation::Right,
            camera: Camera::default(),
            gesture: None,
            selection: Selection::default(),
            configured_storage: None,
            simulation: Simulation::default(),
            challenge: None,
            hotbar: default_hotbar(),
            active_hotbar_folder: Vec::new(),
            active_hotbar_slot: None,
            hotbar_drag: None,
            confirm_hotbar_reset: false,
            io_label: String::new(),
            compile_error: None,
        }
    }

    fn edit(&mut self, operation: LogicGridOperation) {
        self.store.operate(&[operation]);
        self.observed_revision = None;
        self.sync(None, Uuid::nil());
    }

    fn edit_all(&mut self, operations: impl IntoIterator<Item = LogicGridOperation>) {
        let operations = operations.into_iter().collect::<Vec<_>>();
        if operations.is_empty() {
            return;
        }
        self.store.operate(&operations);
        self.observed_revision = None;
        self.sync(None, Uuid::nil());
    }

    fn place(
        &mut self,
        position: Point,
        orientation: ComponentOrientation,
        kind: ComponentKind,
    ) -> ComponentId {
        let id = self.grid.next_component_id();
        self.edit(LogicGridOperation::AddComponent {
            component: Component {
                id,
                position,
                orientation,
                kind,
            },
        });
        id
    }

    fn toggle_storage_bit(&mut self, id: ComponentId, bit: u32) {
        let Some(Component {
            kind: ComponentKind::Storage { scale, value },
            ..
        }) = self.grid.component(id)
        else {
            return;
        };
        if bit >= scale.get() as u32 {
            return;
        }
        let value = value ^ (1_u64 << bit);
        self.edit(LogicGridOperation::SetStorageValue { id, value });
    }

    fn sync(&mut self, client: Option<&BlockClient>, client_id: Uuid) -> bool {
        let refreshed = client.is_some() && self.refresh_compiled();
        let Some(revision) = self.store.revision() else {
            return false;
        };
        if self.observed_revision == Some(revision) {
            return self.sync_hotbar(client, client_id) || refreshed;
        }
        let Some((grid, challenge, called)) = self.store.read(|content| {
            let root = content.root();
            (root.grid(), root.challenge, root.called_blocks())
        }) else {
            return false;
        };
        self.grid = grid;
        self.observed_revision = Some(revision);

        if self.challenge.as_ref().map(|state| state.id) != challenge {
            self.challenge = challenge.map(|id| ChallengeState {
                id,
                data: generate_challenge(id),
                test: ChallengeTest::default(),
                passed_event: false,
            });
        }
        if client.is_some() {
            for compiled in called {
                self.ensure_compiled(compiled);
            }
        }
        self.sync_hotbar(client, client_id);
        true
    }

    fn loaded(&self) -> bool {
        self.observed_revision.is_some()
    }

    fn ensure_compiled(&mut self, compiled: Uuid) {
        if self.compiled.contains_key(&compiled) {
            return;
        }
        let Some(editor) = self.store.editor() else {
            return;
        };
        let content = editor.content_of::<CompiledLogicContent>(compiled);
        self.compiled.insert(
            compiled,
            CompiledSource {
                content,
                revision: None,
                program: None,
            },
        );
    }

    fn refresh_compiled(&mut self) -> bool {
        let mut changed = false;
        let mut called = Vec::new();
        for source in self.compiled.values_mut() {
            let revision = source.content.revision();
            if revision.is_none() || revision == source.revision {
                continue;
            }
            source.revision = revision;
            source.program = source
                .content
                .read(|content| content.field(ObjectId::ROOT, CompiledLogicDocument::COMPILED))
                .flatten();
            called.extend(
                source
                    .program
                    .iter()
                    .flat_map(|program| program.calls().to_vec()),
            );
            changed = true;
        }
        for compiled in called {
            self.ensure_compiled(compiled);
        }
        if changed {
            self.simulation.snapshot = None;
        }
        changed
    }

    fn compiled_kind(&self, compiled: Uuid, name: &str) -> Option<ComponentKind> {
        self.compiled
            .get(&compiled)?
            .program
            .as_ref()?
            .placement(compiled, name)
            .ok()
    }

    fn compile(&mut self, client: &BlockClient) -> Option<(Uuid, Uuid)> {
        let editor = self.store.editor()?.clone();
        let source_id = editor.block_id();
        let compiled = dynamic_artifact::generate_initial(source_id, &self.grid);
        match compiled {
            Ok(compiled) => {
                let child = client.create_dynamic_artifact(
                    CompiledBlock::new(),
                    dynamic_artifact::descriptor(source_id),
                );
                editor.seed_content(
                    child.id(),
                    &CompiledLogicContent::new(&CompiledLogicDocument::of(compiled.clone())),
                );
                let source_name = client
                    .get_block::<LogicGrid>(source_id)
                    .name()
                    .unwrap_or_else(|| DISPLAY_NAME.to_owned());
                let name = dynamic_artifact::artifact_name(&source_name);
                child.set_name(name.clone());
                let id = child.id();

                self.ensure_compiled(id);
                if let Some(source) = self.compiled.get_mut(&id) {
                    source.program = Some(compiled);
                }
                self.pin_component(name, id);
                self.compile_error = None;
                Some((id, CompiledBlock::TYPE_ID))
            }
            Err(error) => {
                self.compile_error = Some(error);
                None
            }
        }
    }
}

#[cfg(test)]
impl LogicGridEditor {
    fn detached(grid: Grid, challenge: Option<ChallengeId>) -> Self {
        let mut editor = Self::new(GridStore::Local {
            content: LogicGridContent::new(
                &block_editor_plugin::be_block::LogicGridDocument::with_grid(&grid, challenge),
            ),
            revision: 0,
        });
        editor.sync(None, Uuid::nil());
        editor
    }

    fn seed<R>(&mut self, build: impl FnOnce(&mut Grid) -> R) -> R {
        let mut grid = self.grid.clone();
        let result = build(&mut grid);
        let challenge = self.challenge.as_ref().map(|state| state.id);
        self.store = GridStore::Local {
            content: LogicGridContent::new(
                &block_editor_plugin::be_block::LogicGridDocument::with_grid(&grid, challenge),
            ),
            revision: self.store.revision().unwrap_or(0) + 1,
        };
        self.observed_revision = None;
        self.sync(None, Uuid::nil());
        result
    }
}

#[cfg(test)]
impl Default for LogicGridEditor {
    fn default() -> Self {
        Self::detached(Grid::new(), None)
    }
}

#[cfg(test)]
pub(crate) fn descriptor_data(source: Uuid) -> Vec<u8> {
    dynamic_artifact::descriptor(source).data
}

#[cfg(test)]
pub(crate) fn artifact_summary(data: &[u8]) -> String {
    dynamic_artifact::summary(data)
}

#[cfg(test)]
mod tests;
