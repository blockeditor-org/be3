use foldhash::{HashMap, HashSet};
use reactive::Get;
use reactive::Set;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Instant;

use accesskit::Node;

use crate::accessibility::{self, AccessibilityTree};
use crate::child_list::{ChildHost, NodeChildren, SlotId};
use crate::context::{Context, Moved};
use crate::damage::{Damage, Region};
use crate::file_picker::{FileFilter, FilePick, FilePickId};
use crate::file_saver::{FileSave, FileSaveId, SavedFile};
use crate::flash::FlashLog;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::input::{Key, KeyChord, Modifiers};
use crate::window::hosted::Painting;
use crate::window::{Appearance, WindowId};

use crate::display::Display;
use crate::interact;
use crate::layout;
use crate::node::{
    ANCESTOR_LIMIT, Arena, Element, NodeId, NodeMap, NodeOf, Placed, Rects, SpaceId,
};
use crate::paint::{self, PaintCache, verify};
use crate::painter::{Painter, PainterState, Zoom};
use crate::performance::{FrameMeasurement, FrameWork, PerformanceSnapshot, PerformanceTracker};
use crate::pixel_grid::PixelGrid;
use crate::screen_simulation::{self, Placement};
use crate::sight::Sight;

type PickedCallback = Box<dyn FnOnce(FilePick)>;
type SavedCallback = Box<dyn FnOnce(FileSave)>;
pub type FingerTap = dyn Fn(usize) -> bool;

pub type ActionsHook = dyn Fn(&Context, &[NodeId], &[WindowId], &[(WindowId, WindowId)]);

pub trait Tools: Any {
    fn show(&mut self, document: &mut Document, ctx: &Context, rect: Rect);

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Acted {
    #[default]
    Nothing,
    Pointer(Pos2),
    Keys,
}

pub struct Document {
    pub arena: Arena,
    pub root: Option<NodeId>,
    appearance: Option<Appearance>,
    pub focused: Option<NodeId>,
    pub(crate) focus_before: Option<NodeId>,
    pub composed: String,
    pub(crate) keyboard_held: bool,
    touched: ::reactive::OwnedSignal<bool>,
    pub(crate) finger_pressed: bool,
    pub(crate) keyboard_asks: u32,
    pub activated: Option<NodeId>,
    pub activation_key: Option<Key>,
    pub rects: Rc<Rects>,
    tools: Option<Box<dyn Tools>>,
    keyboard_host: Option<Box<dyn crate::software_keyboard::KeyboardHost>>,
    inspector_requested: bool,
    screen_pointer: Option<Pos2>,
    pub last_pointer: Option<crate::input::PointerSample>,
    pub acted: Acted,
    placement: Option<(Rect, Option<Placement>)>,
    pub portal_holders: foldhash::HashMap<NodeId, NodeOf<crate::base::portal::PortalNode>>,
    pub(crate) layer_state: crate::layer::OpenLayers,
    pub(crate) focus_scopes: crate::focus_scope::FocusScopes,
    pub(crate) focus_dirty: bool,
    pub(crate) focus_wanted: Option<NodeId>,
    pub(crate) focus_into: Option<(NodeId, u64)>,
    pub(crate) back_handlers: crate::back::BackHandlers,
    pub(crate) back_gesture: Option<crate::layer::DismissTarget>,
    timers: Rc<RefCell<crate::timer::Timers>>,
    scale: ::reactive::OwnedSignal<f32>,
    attached: ::reactive::OwnedSignal<u64>,
    focus_visible: ::reactive::OwnedSignal<bool>,
    focus_watched: ::reactive::OwnedSignal<Option<NodeId>>,
    fullscreen: ::reactive::OwnedSignal<bool>,
    viewport_size: ::reactive::OwnedSignal<Vec2>,
    reattached: Cell<bool>,
    pub(crate) pointer_lockers: RefCell<Vec<NodeOf<crate::base::interactive::InteractiveNode>>>,
    pub(crate) pointer_locked: Cell<bool>,
    finger_taps: RefCell<Vec<Weak<FingerTap>>>,
    pub keys: crate::keys::KeyListeners,
    actions_hooks: RefCell<Vec<Weak<ActionsHook>>>,
    pub(crate) tapping: Option<Key>,
    keyboard: ::reactive::OwnedSignal<bool>,
    input_frames: u64,
    modifiers: ::reactive::OwnedSignal<Modifiers>,
    intercepted_keys: ::reactive::OwnedSignal<Vec<KeyChord>>,
    pub(crate) press_claimants: HashSet<NodeId>,
    pub(crate) drawing_surfaces: HashSet<NodeId>,
    pub(crate) every_sample_surfaces: HashSet<NodeId>,
    pub(crate) outside_watchers: HashSet<NodeId>,
    pub forward: crate::interact::forward::Routing,
    pub pointers: crate::interact::pointer::Pointers,
    pub drags: Rc<crate::drag_board::Board>,
    paste_requested: bool,
    unsent_file_picks: Vec<(FileFilter, PickedCallback)>,
    waiting_file_picks: Vec<(FilePickId, PickedCallback)>,
    unsent_file_saves: Vec<(SavedFile, SavedCallback)>,
    waiting_file_saves: Vec<(FileSaveId, SavedCallback)>,
    test_ids: HashMap<String, Vec<NodeId>>,
    node_test_ids: HashMap<NodeId, Vec<String>>,
    layout_revision: u64,
    paint_revision: u64,
    reports: Vec<Box<dyn FnOnce()>>,
    invalidated: Vec<NodeId>,
    interaction_done: Option<Rc<dyn Fn()>>,
    laid_out: Option<Rc<dyn Fn()>>,
    measurements: NodeMap<Vec<Measurement>>,
    offer_dependent: bool,
    baselines: NodeMap<Vec<(Vec2, Option<f32>)>>,
    min_widths: NodeMap<f32>,
    layout_parent: Option<NodeId>,
    placed_children: NodeMap<Vec<NodeId>>,
    placing: Vec<NodeId>,
    pub interact_bounds: NodeMap<Rect>,
    pub interact_bounds_version: Option<u64>,
    pub placed_pass: NodeMap<u64>,
    pub reached_pass: NodeMap<u64>,
    layout_pass: u64,
    layout_frame: u64,
    focus_line: FocusLine,
    anchor_shift: f32,
    pinned_to_end: Option<crate::base::list::Direction>,
    pub(crate) reveals: NodeMap<NodeId>,
    viewport: Option<(Context, Rect, f32)>,
    pub(crate) hidden: bool,
    pub(crate) embeds: Vec<std::rc::Weak<crate::base::embed::EmbedState>>,
    fonts_generation: u64,
    painting: Vec<(Rc<Display>, Vec2)>,
    pub paint_cache: RefCell<PaintCache>,
    pub(crate) canvas_moves: Vec<NodeId>,
    pub(crate) canvas_moves_from: u64,
    pub copied_text: Option<String>,
    pub copied_image: Option<Vec<u8>>,
    next_paint: Option<Instant>,
    now: Instant,
    reactive_scope: ::reactive::Scope,
    zone: u64,
    extensions: HashMap<std::any::TypeId, Box<dyn Any>>,
    node_refs: HashMap<NodeId, Vec<Weak<Cell<Option<NodeId>>>>>,
    measured: NodeMap<::reactive::OwnedSignal<Option<beui_types::Measured>>>,
    measured_text: NodeMap<::reactive::OwnedSignal<Option<crate::base::text::TextGeometry>>>,
    measure_pending: bool,
    component_states: HashMap<NodeId, Vec<Box<dyn Any>>>,
    component_names: HashMap<NodeId, ComponentNames>,
    pub accessibility_id: u32,
    pub accessibility: NodeMap<Node>,
    pub accessibility_tree: RefCell<AccessibilityTree>,
    performance: PerformanceTracker,
    pub work: WorkCounters,
    changes: FlashLog<NodeId>,
    damage: Damage,
    damage_flashes: FlashLog<Rect>,
    painters: NodeMap<Placing>,
    motion: ::reactive::OwnedSignal<crate::animation::Motion>,
    theme: ::reactive::OwnedSignal<crate::preferences::ThemeKind>,
    canvas_touch: ::reactive::OwnedSignal<crate::preferences::CanvasTouch>,
    preference_layers: ::reactive::OwnedSignal<crate::preferences::PreferenceLayers>,
    out_of_bounds: Option<Vec<(WindowId, String)>>,
    animations: Rc<Cell<usize>>,
    pub(crate) windows: crate::window::hosted::Windows,
}

#[derive(Clone, Copy)]
struct Placing {
    given: PainterState,
    own: PainterState,
    sight: Sight,
    culled: bool,
}

const REMEMBERED_MEASUREMENTS: usize = 4;

#[derive(Default)]
struct FocusLine {
    key: Option<(u64, NodeId)>,
    epoch: u64,
    ancestors: NodeMap<u64>,
    within: NodeMap<(u64, bool)>,
}

#[derive(Clone, Copy)]
struct Measurement {
    offered: Vec2,
    size: Vec2,
    offer_dependent: bool,
}

fn same_size(left: Vec2, right: Vec2) -> bool {
    left.x.to_bits() == right.x.to_bits() && left.y.to_bits() == right.y.to_bits()
}

fn still_fits(offered: Vec2, size: Vec2, available: Vec2) -> bool {
    axis_still_fits(offered.x, size.x, available.x)
        && axis_still_fits(offered.y, size.y, available.y)
}

fn axis_still_fits(offered: f32, size: f32, available: f32) -> bool {
    if offered.to_bits() == available.to_bits() {
        return true;
    }
    match offered.is_finite() {
        true => available.is_finite() && size <= available && available <= offered,
        false => available.to_bits() == size.to_bits(),
    }
}

#[derive(Default)]
pub struct WorkCounters {
    measured: Cell<usize>,
    reused_measurements: Cell<usize>,
    placed: Cell<usize>,
    reused_placements: Cell<usize>,
    painted_nodes: Cell<usize>,
    replayed_nodes: Cell<usize>,
    described_nodes: Cell<usize>,
    walked_nodes: Cell<usize>,
}

impl WorkCounters {
    fn reset(&self) {
        self.measured.set(0);
        self.reused_measurements.set(0);
        self.placed.set(0);
        self.reused_placements.set(0);
        self.painted_nodes.set(0);
        self.replayed_nodes.set(0);
        self.described_nodes.set(0);
        self.walked_nodes.set(0);
    }

    pub fn note_described(&self, nodes: usize, walked: usize) {
        self.described_nodes.set(self.described_nodes.get() + nodes);
        self.walked_nodes.set(self.walked_nodes.get() + walked);
    }

    fn gathered(&self) -> FrameWork {
        FrameWork {
            measured: self.measured.get(),
            reused_measurements: self.reused_measurements.get(),
            placed: self.placed.get(),
            reused_placements: self.reused_placements.get(),
            painted_nodes: self.painted_nodes.get(),
            replayed_nodes: self.replayed_nodes.get(),
            described_nodes: self.described_nodes.get(),
            walked_nodes: self.walked_nodes.get(),
        }
    }
}

pub struct LayoutFrame {
    parent: Option<NodeId>,
    base: usize,
}

impl LayoutFrame {
    pub fn base(&self) -> usize {
        self.base
    }
}

static NEXT_ZONE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Document {
    pub fn new() -> Self {
        let rects = Rc::new(Rects::default());
        let zone = NEXT_ZONE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            arena: Arena::default(),
            root: None,
            appearance: None,
            focused: None,
            focus_before: None,
            composed: String::new(),
            keyboard_held: false,
            touched: ::reactive::OwnedSignal::new(false),
            finger_pressed: false,
            keyboard_asks: 0,
            activated: None,
            activation_key: None,
            drags: Rc::new(crate::drag_board::Board::new(Rc::clone(&rects))),
            rects,
            tools: None,
            keyboard_host: None,
            inspector_requested: false,
            screen_pointer: None,
            last_pointer: None,
            acted: Acted::Nothing,
            placement: None,
            portal_holders: foldhash::HashMap::default(),
            layer_state: Default::default(),
            focus_scopes: Default::default(),
            focus_dirty: false,
            focus_wanted: None,
            focus_into: None,
            back_handlers: crate::back::BackHandlers::default(),
            back_gesture: None,
            timers: Rc::default(),
            scale: ::reactive::OwnedSignal::new(1.0),
            attached: ::reactive::OwnedSignal::new(0),
            focus_visible: ::reactive::OwnedSignal::new(false),
            focus_watched: ::reactive::OwnedSignal::new(None),
            fullscreen: ::reactive::OwnedSignal::new(false),
            viewport_size: ::reactive::OwnedSignal::new(Vec2::ZERO),
            reattached: Cell::new(false),
            pointer_lockers: RefCell::default(),
            pointer_locked: Cell::new(false),
            finger_taps: RefCell::new(Vec::new()),
            keys: Default::default(),
            actions_hooks: RefCell::new(Vec::new()),
            tapping: None,
            keyboard: ::reactive::OwnedSignal::new(true),
            input_frames: 0,
            modifiers: ::reactive::OwnedSignal::new(Modifiers::NONE),
            intercepted_keys: ::reactive::OwnedSignal::new(Vec::new()),
            press_claimants: HashSet::default(),
            drawing_surfaces: HashSet::default(),
            every_sample_surfaces: HashSet::default(),
            outside_watchers: HashSet::default(),
            forward: Default::default(),
            pointers: Default::default(),
            paste_requested: false,
            unsent_file_picks: Vec::new(),
            waiting_file_picks: Vec::new(),
            unsent_file_saves: Vec::new(),
            waiting_file_saves: Vec::new(),
            test_ids: HashMap::default(),
            node_test_ids: HashMap::default(),
            layout_revision: 0,
            paint_revision: 0,
            reports: Vec::new(),
            invalidated: Vec::new(),
            interaction_done: None,
            laid_out: None,
            measurements: NodeMap::default(),
            offer_dependent: false,
            baselines: NodeMap::default(),
            min_widths: NodeMap::default(),
            layout_parent: None,
            placed_children: NodeMap::default(),
            placing: Vec::new(),
            interact_bounds: NodeMap::default(),
            interact_bounds_version: None,
            placed_pass: NodeMap::default(),
            reached_pass: NodeMap::default(),
            layout_pass: 0,
            layout_frame: 0,
            focus_line: FocusLine::default(),
            anchor_shift: 0.0,
            pinned_to_end: None,
            reveals: NodeMap::default(),
            viewport: None,
            hidden: false,
            embeds: Vec::new(),
            fonts_generation: 0,
            painting: Vec::new(),
            paint_cache: RefCell::new(PaintCache::default()),
            canvas_moves: Vec::new(),
            canvas_moves_from: 0,
            copied_text: None,
            copied_image: None,
            next_paint: None,
            now: Instant::now(),
            reactive_scope: ::reactive::Scope::new(),
            zone,
            extensions: HashMap::default(),
            node_refs: HashMap::default(),
            measured: NodeMap::default(),
            measured_text: NodeMap::default(),
            measure_pending: false,
            component_states: HashMap::default(),
            component_names: HashMap::default(),
            accessibility_id: accessibility::next_document_id(),
            accessibility: NodeMap::default(),
            accessibility_tree: RefCell::default(),
            performance: PerformanceTracker::default(),
            work: WorkCounters::default(),
            changes: FlashLog::default(),
            damage: Damage::default(),
            damage_flashes: FlashLog::default(),
            painters: NodeMap::default(),
            motion: ::reactive::OwnedSignal::new(crate::animation::Motion::default()),
            theme: ::reactive::OwnedSignal::new(crate::preferences::ThemeKind::default()),
            canvas_touch: ::reactive::OwnedSignal::new(Default::default()),
            preference_layers: ::reactive::OwnedSignal::new(Default::default()),
            out_of_bounds: None,
            animations: Rc::default(),
            windows: Default::default(),
        }
    }

    pub fn set_root(&mut self, id: NodeId) {
        if self.root != Some(id) {
            if let Some(previous) = self.root {
                self.forget_placement(previous);
            }
            self.arena.invalidate();
            self.root = Some(id);
        }
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    pub fn appearance(&self) -> Option<&Appearance> {
        self.appearance.as_ref()
    }

    pub fn update_appearance(
        &mut self,
        window: Option<WindowId>,
        change: impl FnOnce(&mut Appearance),
    ) {
        match window {
            Some(window) => change(self.windows.appearances.entry(window).or_default()),
            None => change(self.appearance.get_or_insert_default()),
        }
        if let Some((ctx, _, _)) = &self.viewport {
            ctx.request_repaint();
        }
    }

    fn relay_for_viewport(&mut self) {
        let Some(root) = self.root else {
            return;
        };
        self.arena.invalidate_node(root);
        for overlay in self.open_overlays() {
            self.arena.invalidate_node(overlay);
        }
    }

    pub fn reactive_scope(&self) -> &::reactive::Scope {
        &self.reactive_scope
    }

    pub fn zone(&self) -> u64 {
        self.zone
    }

    pub fn dispose(&mut self) {
        let scope = std::mem::replace(&mut self.reactive_scope, ::reactive::Scope::detached());
        crate::current::with_reactive_scope(self, || scope.dispose());
        ::reactive::forget_zone(self.zone);
    }

    pub fn extension<T: Clone + 'static>(&self) -> Option<T> {
        self.extensions
            .get(&std::any::TypeId::of::<T>())
            .and_then(|held| held.downcast_ref::<T>())
            .cloned()
    }

    pub fn extension_or_insert_with<T: Clone + 'static>(&mut self, make: impl FnOnce() -> T) -> T {
        if let Some(held) = self.extension::<T>() {
            return held;
        }
        let made = self.reactive_scope.context().run(make);
        self.extensions
            .insert(std::any::TypeId::of::<T>(), Box::new(made.clone()));
        made
    }

    pub fn open_inspector(&mut self) {
        self.inspector_requested = true;
    }

    pub fn take_inspector_request(&mut self) -> bool {
        std::mem::take(&mut self.inspector_requested)
    }

    pub fn context(&self) -> Option<&Context> {
        self.viewport.as_ref().map(|(context, _, _)| context)
    }

    pub fn request_repaint_after(&self, delay: std::time::Duration) {
        if let Some((ctx, _, _)) = &self.viewport {
            ctx.request_repaint_after(delay);
        }
    }

    pub fn timers(&self) -> Rc<RefCell<crate::timer::Timers>> {
        Rc::clone(&self.timers)
    }

    pub fn watch_context(&self) -> ::reactive::ReadSignal<u64> {
        self.attached.read_only()
    }

    pub fn watch_pixels_per_point(&self) -> ::reactive::ReadSignal<f32> {
        self.scale.read_only()
    }

    pub fn watch_viewport_size(&self) -> ::reactive::ReadSignal<Vec2> {
        self.viewport_size.read_only()
    }

    pub fn watch_fullscreen(&self) -> ::reactive::ReadSignal<bool> {
        self.fullscreen.read_only()
    }

    pub fn watch_focused(&self) -> ::reactive::ReadSignal<Option<NodeId>> {
        self.focus_watched.read_only()
    }

    pub(crate) fn set_focused(&mut self, focused: Option<NodeId>) {
        let entered = focused.filter(|node| self.focused != Some(*node));
        self.focused = focused;
        self.focus_watched.set(focused);
        if let Some(node) = entered
            && self.focus_takes_text()
        {
            self.reveal_in_canvases(node);
        }
    }

    pub fn watch_focus_visible(&self) -> ::reactive::ReadSignal<bool> {
        self.focus_visible.read_only()
    }

    pub fn set_focus_visible(&self, visible: bool) {
        self.focus_visible.set(visible);
    }

    pub fn has_keyboard(&self) -> bool {
        self.keyboard.get_untracked()
    }

    pub fn watch_keyboard(&self) -> ::reactive::ReadSignal<bool> {
        self.keyboard.read_only()
    }

    pub fn touched(&self) -> bool {
        self.touched.get_untracked()
    }

    pub fn touched_by_finger(&self) -> bool {
        self.finger_pressed && self.touched()
    }

    pub(crate) fn set_touched(&self, touched: bool) {
        if self.touched() != touched {
            self.touched.set(touched);
        }
    }

    pub(crate) fn set_keyboard(&self, keyboard: bool) {
        if self.has_keyboard() != keyboard {
            self.keyboard.set(keyboard);
        }
    }

    pub fn register_finger_tap(&self, tap: Weak<FingerTap>) {
        self.finger_taps.borrow_mut().push(tap);
    }

    pub fn finger_tap(&self, fingers: usize) -> bool {
        let mut taps = self.finger_taps.borrow_mut();
        taps.retain(|tap| tap.strong_count() > 0);
        let live: Vec<Rc<FingerTap>> = taps.iter().filter_map(Weak::upgrade).collect();
        drop(taps);
        live.into_iter().any(|tap| tap(fingers))
    }

    pub fn focus_ancestry(&self) -> Vec<NodeId> {
        let mut path = Vec::new();
        let mut next = self.focused_node();
        while let Some(id) = next {
            path.push(id);
            next = self.arena.parent(id);
        }
        path
    }

    pub fn register_actions(&self, hook: Weak<ActionsHook>) {
        self.actions_hooks.borrow_mut().push(hook);
    }

    fn describe_actions(&self, ctx: &Context) {
        let mut hooks = self.actions_hooks.borrow_mut();
        hooks.retain(|hook| hook.strong_count() > 0);
        let live: Vec<Rc<ActionsHook>> = hooks.iter().filter_map(Weak::upgrade).collect();
        drop(hooks);
        let path = self.focus_ancestry();
        let windows = self.open_windows();
        let hosts = self.window_hosts();
        for hook in live {
            hook(ctx, &path, &windows, &hosts);
        }
    }

    pub fn listen_keys(
        &self,
        at: crate::keys::KeyAt,
        phase: crate::keys::KeyPhase,
        reveal: bool,
        handler: Weak<crate::keys::KeyHandler>,
        window: Option<WindowId>,
    ) {
        self.keys.listen(at, phase, reveal, handler, window);
    }

    pub fn input_frames(&self) -> u64 {
        self.input_frames
    }

    pub(crate) fn note_input(&mut self) {
        self.input_frames += 1;
    }

    pub fn watch_modifiers(&self) -> ::reactive::ReadSignal<Modifiers> {
        self.modifiers.read_only()
    }

    pub fn intercepted_keys_writer(&self) -> ::reactive::Signal<Vec<KeyChord>> {
        self.intercepted_keys.handle()
    }

    pub fn intercepted_keys(&self) -> Vec<KeyChord> {
        self.intercepted_keys.get_untracked()
    }

    pub fn set_modifiers(&self, modifiers: Modifiers) {
        if self.modifiers.get_untracked() != modifiers {
            self.modifiers.set(modifiers);
        }
    }

    fn run_timers(&self) {
        let scale = self.pixels_per_point();
        if self.scale.get_untracked() != scale {
            self.scale.set(scale);
        }
        let fullscreen = self
            .viewport
            .as_ref()
            .is_some_and(|(ctx, _, _)| ctx.fullscreen());
        if self.fullscreen.get_untracked() != fullscreen {
            self.fullscreen.set(fullscreen);
        }
        let size = self
            .viewport
            .as_ref()
            .map_or(Vec2::ZERO, |(_, rect, _)| rect.size());
        if self.viewport_size.get_untracked() != size {
            self.viewport_size.set(size);
        }
        let reattached = self.reattached.replace(false);
        if reattached {
            self.attached.update(|attached| *attached += 1);
        }
        self.publish_pointer_lock(reattached);
        let now = self.now;
        let mut timers = self.timers.borrow_mut();
        timers.retain(|timer| {
            timer
                .upgrade()
                .is_some_and(|timer| timer.due().is_some() || timer.unlist())
        });
        let due: Vec<_> = timers
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|timer| timer.due().is_some_and(|due| due <= now))
            .collect();
        drop(timers);
        for timer in due {
            timer.fire(now);
        }
    }

    fn next_timer(&self) -> Option<Instant> {
        self.timers
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .filter_map(|timer| timer.due())
            .min()
    }

    pub fn register_node_ref(&mut self, node: NodeId, cell: Weak<Cell<Option<NodeId>>>) {
        let refs = self.node_refs.entry(node).or_default();
        refs.retain(|cell| cell.strong_count() > 0);
        refs.push(cell);
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.arena.get(id).children()
    }

    pub fn open_child_slot<H: ChildHost>(&mut self, node: NodeOf<H>) -> SlotId {
        self.arena.get_mut_as::<H>(node).children().open()
    }

    pub fn fill_child_slot<H: ChildHost>(
        &mut self,
        node: NodeOf<H>,
        slot: SlotId,
        items: Vec<H::Stored>,
    ) {
        let host = self.arena.get_mut_as::<H>(node);
        host.children().fill_children(slot, items);
        host.children_changed();
    }

    pub fn append_child_item<H: ChildHost>(&mut self, node: NodeOf<H>, item: H::Stored) {
        let host = self.arena.get_mut_as::<H>(node);
        host.children().push(item);
        host.children_changed();
    }

    pub fn node_kind(&self, id: NodeId) -> &'static str {
        self.arena.get(id).kind()
    }

    pub fn node_detail(&self, id: NodeId) -> Option<String> {
        self.arena.get(id).detail()
    }

    pub fn node_properties(&self, id: NodeId) -> Vec<(&'static str, String)> {
        self.arena.get(id).properties()
    }

    pub fn node_parent(&self, id: NodeId) -> Option<NodeId> {
        self.arena.parent(id)
    }

    pub fn node_test_ids(&self, id: NodeId) -> &[String] {
        self.node_test_ids.get(&id).map_or(&[], Vec::as_slice)
    }

    pub fn node_rect(&self, id: impl Into<NodeId>) -> Option<Rect> {
        let id = id.into();
        self.rects.get(&id)
    }

    pub fn set_test_id(&mut self, id: NodeId, test_id: impl Into<String>) {
        let test_id = test_id.into();
        if test_id.is_empty() {
            return;
        }
        let named = self.test_ids.entry(test_id.clone()).or_default();
        if !named.contains(&id) {
            named.push(id);
        }
        let owned = self.node_test_ids.entry(id).or_default();
        if !owned.iter().any(|existing| existing == &test_id) {
            owned.push(test_id);
            self.arena.touch(id);
        }
    }

    pub fn clear_test_id(&mut self, id: NodeId, test_id: &str) {
        self.forget_test_id(id, test_id);
        self.drop_test_id(id, test_id);
        if self.arena.contains(id) {
            self.arena.touch(id);
        }
    }

    fn drop_test_id(&mut self, id: NodeId, test_id: &str) {
        if let Some(named) = self.test_ids.get_mut(test_id) {
            named.retain(|node| *node != id);
            if named.is_empty() {
                self.test_ids.remove(test_id);
            }
        }
    }

    fn live_nodes(&self) -> HashSet<NodeId> {
        let mut live = HashSet::default();
        let mut pending: Vec<NodeId> = self.root.into_iter().collect();
        while let Some(id) = pending.pop() {
            if self.arena.contains(id) && live.insert(id) {
                pending.extend(self.arena.get(id).live_children());
            }
        }
        live
    }

    fn named_live(&self, test_id: &str, live: &HashSet<NodeId>) -> Vec<NodeId> {
        self.test_ids
            .get(test_id)
            .into_iter()
            .flatten()
            .copied()
            .filter(|node| live.contains(node))
            .collect()
    }

    fn forget_test_id(&mut self, id: NodeId, test_id: &str) {
        if let Some(owned) = self.node_test_ids.get_mut(&id) {
            owned.retain(|existing| existing != test_id);
            if owned.is_empty() {
                self.node_test_ids.remove(&id);
            }
        }
    }

    pub fn find_test_ids(&self, test_id: &str) -> Vec<NodeId> {
        self.named_live(test_id, &self.live_nodes())
    }

    pub fn find_test_id(&self, test_id: &str) -> Option<NodeId> {
        let named = self.find_test_ids(test_id);
        if named.len() > 1 {
            panic!(
                "test id {test_id:?} names {} nodes in the tree; give each its own id",
                named.len()
            );
        }
        named.first().copied()
    }

    pub fn contains(&self, id: impl Into<NodeId>) -> bool {
        let id = id.into();
        self.arena.contains(id)
    }

    pub fn performance(&self) -> PerformanceSnapshot {
        self.performance.snapshot()
    }

    pub fn reset_performance(&mut self) {
        self.performance.clear();
    }

    pub fn track_changes(&mut self, enabled: bool) {
        self.changes.set_enabled(enabled);
    }

    pub fn motion(&self) -> crate::animation::Motion {
        self.motion.get_untracked()
    }

    pub fn animations(&self) -> Rc<Cell<usize>> {
        Rc::clone(&self.animations)
    }

    pub fn watch_motion(&self) -> ::reactive::ReadSignal<crate::animation::Motion> {
        self.motion.read_only()
    }

    pub fn preferences(&self) -> crate::preferences::Preferences {
        crate::preferences::Preferences {
            theme: self.theme.get_untracked(),
            motion: self.motion.get_untracked(),
            canvas_touch: self.canvas_touch.get_untracked(),
        }
    }

    pub fn watch_canvas_touch(&self) -> ::reactive::ReadSignal<crate::preferences::CanvasTouch> {
        self.canvas_touch.read_only()
    }

    pub fn watch_theme(&self) -> ::reactive::ReadSignal<crate::preferences::ThemeKind> {
        self.theme.read_only()
    }

    pub fn watch_preference_layers(
        &self,
    ) -> ::reactive::ReadSignal<crate::preferences::PreferenceLayers> {
        self.preference_layers.read_only()
    }

    fn follow_preferences(&mut self, layers: crate::preferences::PreferenceLayers) {
        if self.preference_layers.get_untracked() == layers {
            return;
        }
        let preferences = layers.resolve();
        crate::current::run_in(self, |document| {
            document.preference_layers.set(layers);
            if document.motion() != preferences.motion {
                document.arena.invalidate();
                document.motion.set(preferences.motion);
            }
            if document.theme.get_untracked() != preferences.theme {
                document.theme.set(preferences.theme);
            }
            if document.canvas_touch.get_untracked() != preferences.canvas_touch {
                document.canvas_touch.set(preferences.canvas_touch);
            }
        });
    }

    pub fn track_damage(&mut self, enabled: bool) {
        self.damage_flashes.set_enabled(enabled);
    }

    pub fn change_flashes(&self) -> impl Iterator<Item = (NodeId, Instant)> {
        self.changes.entries().map(|(id, at)| (*id, at))
    }

    pub fn damage_flashes(&self) -> impl Iterator<Item = (Rect, Instant)> {
        self.damage_flashes.entries().map(|(rect, at)| (*rect, at))
    }

    pub fn flashing(&self) -> bool {
        !self.changes.is_empty() || !self.damage_flashes.is_empty()
    }

    pub fn set_accessibility(&mut self, id: NodeId, node: Node) {
        if self.accessibility.get(&id) != Some(&node) {
            self.accessibility.insert(id, node);
            self.arena.touch(id);
        }
    }

    pub fn copy_text(&mut self, text: impl Into<String>) {
        self.copied_image = None;
        self.copied_text = Some(text.into());
    }

    pub fn copy_image(&mut self, png: Vec<u8>) {
        self.copied_text = None;
        self.copied_image = Some(png);
    }

    pub fn request_paste(&mut self) {
        self.paste_requested = true;
    }

    pub(crate) fn paste_requested(&self) -> bool {
        self.paste_requested
    }

    pub fn pick_file(&mut self, filter: FileFilter, picked: impl FnOnce(FilePick) + 'static) {
        self.unsent_file_picks.push((filter, Box::new(picked)));
    }

    pub fn save_file(&mut self, file: SavedFile, saved: impl FnOnce(FileSave) + 'static) {
        self.unsent_file_saves.push((file, Box::new(saved)));
    }

    fn send_file_dialogs(&mut self, ctx: &Context) {
        for (filter, picked) in std::mem::take(&mut self.unsent_file_picks) {
            let id = ctx.pick_file(filter);
            self.waiting_file_picks.push((id, picked));
        }
        for (file, saved) in std::mem::take(&mut self.unsent_file_saves) {
            let id = ctx.save_file(file);
            self.waiting_file_saves.push((id, saved));
        }
    }

    fn deliver_file_dialogs(&mut self, ctx: &Context) {
        if self.waiting_file_picks.is_empty() && self.waiting_file_saves.is_empty() {
            return;
        }
        let mut delivered: Vec<Box<dyn FnOnce()>> = Vec::new();
        for (id, picked) in std::mem::take(&mut self.waiting_file_picks) {
            match ctx.take_file_pick(id) {
                Some(pick) => delivered.push(Box::new(move || picked(pick))),
                None => self.waiting_file_picks.push((id, picked)),
            }
        }
        for (id, saved) in std::mem::take(&mut self.waiting_file_saves) {
            match ctx.take_file_save(id) {
                Some(save) => delivered.push(Box::new(move || saved(save))),
                None => self.waiting_file_saves.push((id, saved)),
            }
        }
        if delivered.is_empty() {
            return;
        }
        let context = self.reactive_scope().context();
        let _guard = crate::current::install(self);
        context.run(|| {
            for deliver in delivered {
                deliver();
            }
        });
    }

    pub(crate) fn drain_removals(&mut self) {
        while self.arena.removals_pending() {
            let context = self.reactive_scope().context();
            let removals = self.arena.take_removals();
            let guard = crate::current::install(self);
            let installed = matches!(guard, crate::current::DocumentGuard::Installed(..));
            match installed {
                true => {
                    context.run(|| {
                        crate::current::with_document(|document| document.remove_nodes(&removals))
                    });
                    drop(guard);
                }
                false => {
                    drop(guard);
                    self.remove_nodes(&removals);
                }
            }
            self.arena.give_back_removals(removals);
        }
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.remove_nodes(&[id]);
    }

    fn remove_nodes(&mut self, ids: &[NodeId]) {
        let leaving: HashSet<NodeId> = ids.iter().copied().collect();
        let mut parents: HashMap<NodeId, Vec<NodeId>> = HashMap::default();
        for &id in ids {
            if let Some(parent) = self
                .arena
                .parent(id)
                .filter(|parent| !leaving.contains(parent))
            {
                parents.entry(parent).or_default().push(id);
            }
        }
        for &id in ids {
            self.remove_subtree(id);
        }
        for (parent, children) in parents {
            self.unlink_children(parent, &children);
        }
    }

    fn unlink_children(&mut self, parent: NodeId, children: &[NodeId]) {
        if !self.arena.contains(parent) {
            return;
        }
        let held: HashSet<NodeId> = self.arena.get(parent).children().into_iter().collect();
        let children: Vec<NodeId> = children
            .iter()
            .copied()
            .filter(|child| held.contains(child))
            .collect();
        if children.is_empty() {
            return;
        }
        let element = self.arena.get_mut(parent);
        for child in children {
            element.remove_child(child);
        }
    }

    fn remove_subtree(&mut self, id: NodeId) {
        if !self.arena.contains(id) {
            return;
        }
        let mut dropped = Vec::new();
        let rects = Rc::clone(&self.rects);
        self.drop_placement(id, &rects, &mut dropped);
        let returning = self.returns_on_removal(id);
        if self
            .focused
            .is_some_and(|focused| self.is_within(focused, id))
        {
            self.set_focused(None);
        }
        let mut removed = Vec::new();
        self.detach_subtree(id, &mut removed);
        for node in dropped {
            self.release_placement(node);
        }
        if let Some(returning) = returning {
            self.return_removed_focus(returning);
        }
        drop(removed);
    }

    fn detach_subtree(&mut self, id: NodeId, removed: &mut Vec<Box<dyn Element>>) {
        if !self.arena.contains(id) {
            return;
        }
        let element = self.arena.get(id);
        let borrowed = element.borrowed();
        let children = element.children();
        let held: Vec<NodeId> = borrowed
            .iter()
            .copied()
            .filter(|child| self.portal_holders.get(child).map(|portal| portal.id()) == Some(id))
            .collect();
        self.release_portal(id, &borrowed);
        for child in held {
            self.release_forgotten(child);
        }
        for child in children {
            if borrowed.contains(&child) || self.arena.owned(child) {
                continue;
            }
            self.detach_subtree(child, removed);
        }
        let mut element = self.arena.take(id);
        element.detached();
        self.arena.put_back(id, element);
        removed.extend(self.arena.remove(id));
        self.forget_layer(id);
        self.forget_focus_scope(id);
        self.forget_back_gesture(id);
        self.paint_cache.borrow_mut().forget(id);
        self.measured.remove(&id);
        self.measured_text.remove(&id);
        self.measurements.remove(&id);
        self.baselines.remove(&id);
        self.min_widths.remove(&id);
        self.component_states.remove(&id);
        self.component_names.remove(&id);
        self.placed_children.remove(&id);
        self.placed_pass.remove(&id);
        self.press_claimants.remove(&id);
        self.drawing_surfaces.remove(&id);
        self.every_sample_surfaces.remove(&id);
        self.outside_watchers.remove(&id);
        self.reached_pass.remove(&id);
        self.reveals.remove(&id);
        self.accessibility.remove(&id);
        self.accessibility_tree.get_mut().forget(id, &self.arena);
        for test_id in self.node_test_ids.remove(&id).unwrap_or_default() {
            self.drop_test_id(id, &test_id);
        }
        for cell in self.node_refs.remove(&id).unwrap_or_default() {
            if let Some(cell) = cell.upgrade()
                && cell.get() == Some(id)
            {
                cell.set(None);
            }
        }
        if self.root == Some(id) {
            self.root = None;
        }
        if self.focused == Some(id) {
            self.set_focused(None);
        }
        if self.focus_before == Some(id) {
            self.focus_before = None;
        }
        if self.focus_wanted == Some(id) {
            self.focus_wanted = None;
        }
        if self.activated == Some(id) {
            self.activated = None;
        }
    }

    pub fn show(&mut self, ctx: &Context, rect: Rect) {
        self.show_with_tools(ctx, rect);
        if let Some(appearance) = &self.appearance {
            ctx.set_appearance(appearance);
        }
    }

    fn show_with_tools(&mut self, ctx: &Context, rect: Rect) {
        if let Some(mut tools) = self.tools.take() {
            tools.show(self, ctx, rect);
            if self.tools.is_none() {
                self.tools = Some(tools);
            }
            return;
        }
        self.track_changes(false);
        self.track_damage(false);
        let content = trim_bottom(rect, ctx.measure_mouse_simulation(rect)).0;
        crate::software_keyboard::show_with_keyboard(self, ctx, content, |document, content| {
            document.show_screen(ctx, content)
        });
        ctx.show_mouse_simulation(rect);
    }

    pub fn on_interacted(&mut self, interacted: impl Fn() + 'static) {
        self.interaction_done = Some(Rc::new(interacted));
    }

    pub fn on_laid_out(&mut self, laid_out: impl Fn() + 'static) {
        self.laid_out = Some(Rc::new(laid_out));
    }

    pub fn set_keyboard_host(&mut self, host: Box<dyn crate::software_keyboard::KeyboardHost>) {
        self.keyboard_host = Some(host);
    }

    pub(crate) fn take_keyboard_host(
        &mut self,
    ) -> Option<Box<dyn crate::software_keyboard::KeyboardHost>> {
        self.keyboard_host.take()
    }

    pub fn set_tools(&mut self, tools: Box<dyn Tools>) {
        self.tools = Some(tools);
    }

    pub fn tools<T: 'static>(&self) -> Option<&T> {
        self.tools.as_ref()?.as_any().downcast_ref::<T>()
    }

    pub fn tools_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.tools.as_mut()?.as_any_mut().downcast_mut::<T>()
    }

    pub fn hide(&mut self) {
        if !std::mem::replace(&mut self.hidden, true) {
            self.conceal_embeds(true);
        }
    }

    fn conceal_embeds(&mut self, concealed: bool) {
        self.embeds.retain(|embed| match embed.upgrade() {
            Some(embed) => {
                embed.conceal(concealed);
                true
            }
            None => false,
        });
    }

    pub fn show_screen(&mut self, ctx: &Context, rect: Rect) {
        if let Some(pos) = ctx
            .input(|input| input.pointer.pos)
            .filter(|pos| rect.contains(*pos))
        {
            self.screen_pointer = Some(pos);
        }
        let placement = ctx.screen_simulation().and_then(|simulation| {
            simulation.place(rect, self.screen_pointer, ctx.pixels_per_point())
        });
        let previous = self.placement.replace((rect, placement));
        if previous != Some((rect, placement)) {
            ctx.report_damage(rect);
        }
        if previous.and_then(|(_, previous)| previous) != placement {
            ctx.request_repaint();
        }
        ctx.set_screen_scale(placement.map_or(1.0, |placement| placement.scale));
        match placement {
            Some(placement) => {
                let shown = placement.shown();
                let painter = ctx.painter();
                for backdrop in screen_simulation::around(rect, shown)
                    .into_iter()
                    .filter(Rect::is_positive)
                {
                    painter.rect_filled(backdrop, 0.0, screen_simulation::BACKDROP);
                }
                ctx.clipped(shown.intersect(rect), || {
                    ctx.scaled(placement.scale, || {
                        self.show_content(ctx, placement.screen);
                    });
                });
            }
            None => self.show_content(ctx, rect),
        }
    }

    pub fn screen_scale(&self) -> f32 {
        self.shown_screen().map_or(1.0, |placement| placement.scale)
    }

    pub fn shown_screen(&self) -> Option<Placement> {
        self.placement.and_then(|(_, placement)| placement)
    }

    pub fn now(&self) -> Instant {
        self.context()
            .map_or(self.now, |context| context.now().max(self.now))
    }

    pub fn show_content(&mut self, ctx: &Context, rect: Rect) {
        self.now = ctx.now();
        self.follow_preferences(ctx.preference_layers());
        if std::mem::take(&mut self.hidden) {
            ctx.report_damage(rect);
            self.conceal_embeds(false);
        }
        let mut measurement = FrameMeasurement::new();
        self.work.reset();
        let scale = ctx.pixels_per_point();
        let fonts_generation = ctx.fonts_generation();
        let resized = self
            .viewport
            .as_ref()
            .is_some_and(|(old_ctx, old_rect, _)| {
                ctx.same(old_ctx) && old_rect.size() != rect.size()
            });
        let refonted =
            std::mem::replace(&mut self.fonts_generation, fonts_generation) != fonts_generation;
        if refonted
            || self
                .viewport
                .as_ref()
                .is_none_or(|(old_ctx, old_rect, old_scale)| {
                    !ctx.same(old_ctx) || *old_rect != rect || *old_scale != scale
                })
        {
            let reattached = refonted
                || self
                    .viewport
                    .as_ref()
                    .is_none_or(|(old_ctx, _, _)| !ctx.same(old_ctx));
            self.reattached.set(self.reattached.get() || reattached);
            let only_moved = !reattached
                && self
                    .viewport
                    .as_ref()
                    .is_some_and(|(_, _, old_scale)| *old_scale == scale);
            match only_moved {
                true => self.relay_for_viewport(),
                false => self.arena.invalidate(),
            }
            if self
                .viewport
                .as_ref()
                .is_none_or(|(old_ctx, _, _)| !ctx.same(old_ctx))
            {
                ::reactive::wake_zone_with(self.zone, ctx.zone_waker(self.zone));
            }
            self.viewport = Some((ctx.clone(), rect, scale));
        }
        self.follow_windows(ctx);
        FrameMeasurement::measure(&mut measurement.timings.accessibility, || {
            let actions = ctx.take_accessibility_actions(self.accessibility_id);
            if !actions.is_empty() {
                crate::current::run_in(self, |document| {
                    for request in actions {
                        document.handle_accessibility_action(request);
                    }
                });
            }
        });

        crate::current::run_in(self, |document| document.run_timers());
        if ctx.automated() {
            crate::current::run_in(self, |document| document.describe_actions(ctx));
        }
        self.deliver_file_dialogs(ctx);

        {
            FrameMeasurement::measure(&mut measurement.timings.interaction, || {
                if let Some(root) = self.root {
                    let rects = Rc::clone(&self.rects);
                    interact::interact(self, ctx, &rects, root);
                }
            });
        }
        if let Some(interacted) = self.interaction_done.clone() {
            crate::current::enter(self, || interacted());
        }
        if self.handles_back() {
            ctx.handle_back();
        }
        let layers = self.layers();
        let focused_modal = self
            .focused
            .and_then(|focused| self.layer_of(&layers, focused))
            .is_some_and(|layer| {
                layer > 0 && layers[layer].policy != crate::layer::LayerPolicy::Pass
            });
        if layers.takes_keyboard() || focused_modal {
            ctx.want_keyboard();
        }
        self.intercepted_keys
            .read_only()
            .with_untracked(|chords| ctx.intercept_keys(chords));

        if let Some(text) = self.copied_text.take() {
            ctx.copy_text(text);
        }
        if let Some(png) = self.copied_image.take() {
            ctx.copy_image(png);
        }
        if std::mem::take(&mut self.paste_requested) {
            ctx.request_paste();
        }
        ctx.accept_pastes(&self.paste_kinds());
        ctx.take_clipboard_keys(self.clipboard_keys());
        let window_resized = std::mem::take(&mut self.windows.focus_resized);
        if (resized || window_resized) && self.focus_takes_text() {
            self.reveal_focus();
        }
        measurement.layout_passes =
            FrameMeasurement::measure(&mut measurement.timings.layout, || {
                self.update_layout(ctx, rect)
            });
        self.ask_for_windows(ctx);
        ctx.set_keyboard_window(
            self.focused
                .map_or(WindowId::MAIN, |focused| self.window_of(focused)),
        );
        if self.has_keyboard()
            && let Some(area) = self.focused_ime_area()
        {
            ctx.set_ime_area(Some(area));
        }
        let now = self.now;
        self.changes.prune(now);
        self.damage_flashes.prune(now);
        if self.arena.take_everything() {
            self.damage.everything();
            self.paint_cache.get_mut().clear();
        }
        let released = self.arena.take_released();
        for id in self.arena.take_changed() {
            self.accessibility_tree.get_mut().mark(id, &self.arena);
            self.changes.record(id, now);
        }
        let mut repaints = self.arena.take_repaints();
        let cache = self.paint_cache.get_mut();
        repaints.extend(cache.take_due(now));
        cache.mark(&repaints, &self.arena);
        self.arena.recycle(released);
        let mut window_damage = None;
        let mut window_moves = Vec::new();
        if !repaints.is_empty() || self.paint_revision != self.arena.revision {
            measurement.painted = true;
            let verifying = verifying_paint();
            let previous =
                verifying.then(|| (self.painting.clone(), self.windows.paintings.clone()));
            FrameMeasurement::measure(&mut measurement.timings.paint, || self.paint(ctx));
            self.paint_revision = self.arena.revision;
            let cache = self.paint_cache.get_mut();
            let moves = cache.take_moves();
            let stale = cache.take_stale();
            let mut taken = self.damage.take();
            let everything = taken.is_everything();
            let region = taken.of(WindowId::MAIN, rect);
            let (region, moved) = match everything {
                true => (region, None),
                false => self.settle_moves(WindowId::MAIN, &moves, &stale, region, rect),
            };
            if !everything {
                for window in self.open_windows() {
                    let Some(viewport) = self.window_rect(window) else {
                        continue;
                    };
                    let held = taken.of(window, viewport);
                    let (settled, moved) =
                        self.settle_moves(window, &moves, &stale, held, viewport);
                    taken.set(window, settled);
                    if let Some(moved) = moved {
                        window_moves.push((window, moved));
                    }
                }
            }
            if let Some((painting, windows)) = previous {
                let mut checks = vec![verify::Check {
                    window: WindowId::MAIN,
                    viewport: rect,
                    previous: painting,
                    region,
                    moved,
                }];
                for (window, painting) in windows {
                    let Some(viewport) = self.window_rect(window) else {
                        continue;
                    };
                    checks.push(verify::Check {
                        window,
                        viewport,
                        previous: painting,
                        region: taken.of(window, viewport),
                        moved: window_moves
                            .iter()
                            .find(|(held, _)| *held == window)
                            .map(|(_, moved)| *moved),
                    });
                }
                self.verify_paint(ctx, &checks, everything);
            }
            window_damage = Some(taken);
            for damaged in region.rects() {
                self.damage_flashes.record(*damaged, now);
                ctx.report_damage(*damaged);
            }
            if let Some(moved) = moved {
                let roots = self.painting.iter().map(|(display, _)| Rc::clone(display));
                ctx.report_move(moved, roots.collect());
            }
            self.next_paint = self.paint_cache.get_mut().next_deadline();
        }
        if let Some(deadline) = self.next_paint {
            ctx.request_repaint_after(deadline.saturating_duration_since(self.now));
        }
        if let Some(deadline) = self.next_timer() {
            ctx.request_repaint_after(deadline.saturating_duration_since(self.now));
        }
        if self.animations.get() > 0 {
            ctx.note_animating();
        }
        ctx.show_painting(&self.painting, rect);
        self.show_windows(ctx, window_damage.as_ref(), &window_moves);
        if ctx.automated() {
            if measurement.painted || measurement.layout_passes > 0 || self.out_of_bounds.is_none()
            {
                let found = self
                    .overflows()
                    .iter()
                    .map(|found| (self.window_of(found.node), found.to_string()))
                    .collect();
                self.out_of_bounds = Some(found);
            }
            ctx.report_overflows(self.out_of_bounds.as_deref().unwrap_or_default());
        }
        self.send_file_dialogs(ctx);
        FrameMeasurement::measure(&mut measurement.timings.accessibility, || {
            if !ctx.accessibility_active() {
                let mut tree = self.accessibility_tree.borrow_mut();
                match tree.take_inspected() {
                    true => tree.discard_changes(),
                    false => tree.reset(),
                }
                return;
            }
            let full = !ctx.accessibility_known(self.accessibility_id);
            if let Some(fragment) = self.accessibility_update(full) {
                ctx.publish_accessibility(self.accessibility_id, fragment);
            }
        });
        measurement.work = self.work.gathered();
        let shapes = self.painting.iter().map(|(display, _)| display.count).sum();
        let frame = measurement.finish(self.arena.len(), shapes);
        self.performance.record(frame);
    }

    fn paint(&mut self, ctx: &Context) {
        let mut roots = Vec::new();
        if let Some(root) = self.root.filter(|_| ctx.shows_window(ctx.drawing_window())) {
            self.paint_cache.get_mut().paint_into(WindowId::MAIN);
            paint::paint(
                self,
                &ctx.window_painter(ctx.drawing_window()),
                &self.rects,
                root,
            );
            roots.push((WindowId::MAIN, root));
        }
        let layers = self.layers();
        let open = self.open_windows();
        for layer in layers.iter().skip(1) {
            let Some(overlay) = layer.overlay else {
                continue;
            };
            let shown = match layer.window.is_main() {
                true => ctx.shows_main(),
                false => open.contains(&layer.window),
            };
            if !shown {
                continue;
            }
            let scrim = self.overlay_scrim(overlay);
            let content = self.overlay_content(overlay);
            let beak = self.overlay_beak(overlay);
            let painter = ctx.window_painter(layer.window);
            self.paint_cache.get_mut().paint_into(layer.window);
            for root in [Some(scrim), content, beak].into_iter().flatten() {
                if self.rects.contains_key(&root) {
                    paint::paint(self, &painter, &self.rects, root);
                    roots.push((layer.window, root));
                }
            }
        }
        let cache = self.paint_cache.get_mut();
        cache.settle_roots(&roots);
        if cache.take_recorded() {
            let mut main = Vec::new();
            let mut windows: Vec<(WindowId, Painting)> = Vec::new();
            for (window, root) in &roots {
                let Some(painted) = cache.painting_of(*root) else {
                    continue;
                };
                match window.is_main() {
                    true => main.push(painted),
                    false => match windows.iter_mut().find(|(id, _)| id == window) {
                        Some((_, painting)) => painting.push(painted),
                        None => windows.push((*window, vec![painted])),
                    },
                }
            }
            self.painting = main;
            self.windows.paintings = windows;
        }
        self.damage.add_regions(cache.take_damage());
    }

    fn settle_moves(
        &self,
        window: WindowId,
        moves: &[paint::Move],
        stale: &[paint::Stale],
        region: Region,
        viewport: Rect,
    ) -> (Region, Option<Moved>) {
        let moves: Vec<&paint::Move> = moves
            .iter()
            .filter(|moved| moved.window == window)
            .collect();
        let stale: Vec<&paint::Stale> = stale
            .iter()
            .filter(|stale| stale.window == window)
            .collect();
        let painting = match window.is_main() {
            true => self.painting.as_slice(),
            false => self
                .windows
                .paintings
                .iter()
                .find(|(held, _)| *held == window)
                .map_or(&[][..], |(_, painting)| painting.as_slice()),
        };
        let shown_area = |moved: &paint::Move| {
            let visible = moved.viewport.intersect(viewport);
            visible.width().max(0.0) * visible.height().max(0.0)
        };
        let Some(largest) =
            (0..moves.len()).max_by(|a, b| shown_area(moves[*a]).total_cmp(&shown_area(moves[*b])))
        else {
            let mut region = region;
            for stale in &stale {
                region.add(stale.rect.intersect(viewport));
            }
            return (region, None);
        };
        let mut region = region;
        for (index, moved) in moves.iter().enumerate() {
            if index != largest {
                region.add(moved.viewport.intersect(viewport));
            }
        }
        let only = moves[largest];
        let visible = only.viewport.intersect(viewport);
        let (fixed, inner) = paint::fixed_damage(painting, only.space, visible, only.by);
        let shown = visible.intersect(inner);
        let landed = shown.intersect(shown.translate(only.by));
        let source = landed.translate(-only.by);
        let cache = self.paint_cache.borrow();
        let mut damaged: Vec<Rect> = region.rects().to_vec();
        for stale in &stale {
            damaged.push(stale.rect);
            if !cache.scrolls_within(stale.scroll, only.space) {
                damaged.push(stale.rect.intersect(source).translate(only.by));
            }
        }
        damaged.extend(fixed.rects().iter().copied());
        let mut settled = Region::NOTHING;
        for rect in paint::uncovered(visible, shown)
            .into_iter()
            .chain(paint::uncovered(shown, landed))
        {
            settled.add(rect);
        }
        for rect in damaged {
            settled.add(rect.intersect(shown));
            for outside in paint::uncovered(rect, visible) {
                settled.add(outside);
            }
        }
        if !landed.is_positive()
            || settled
                .rects()
                .iter()
                .any(|rect| rect.contains_rect(landed))
        {
            settled.add(visible);
            return (settled, None);
        }
        let moved = Moved {
            from: source,
            by: only.by,
        };
        (settled, Some(moved))
    }

    fn verify_paint(&mut self, ctx: &Context, checks: &[verify::Check], everything: bool) {
        let counted = (
            self.work.painted_nodes.get(),
            self.work.replayed_nodes.get(),
        );
        let retained = self.paint_cache.replace(PaintCache::default());
        let painting = std::mem::take(&mut self.painting);
        let windows = std::mem::take(&mut self.windows.paintings);
        let damage = std::mem::take(&mut self.damage);
        ctx.verifying_paint(true);
        self.paint(ctx);
        ctx.verifying_paint(false);
        let fresh_main = std::mem::replace(&mut self.painting, painting);
        let fresh_windows = std::mem::replace(&mut self.windows.paintings, windows);
        self.paint_cache.replace(retained);
        self.damage = damage;
        self.work.painted_nodes.set(counted.0);
        self.work.replayed_nodes.set(counted.1);
        let of = |paintings: &[(WindowId, Painting)], window: WindowId| {
            paintings
                .iter()
                .find(|(held, _)| *held == window)
                .map(|(_, painting)| painting.clone())
                .unwrap_or_default()
        };
        for check in checks {
            let (fresh, retained) = match check.window.is_main() {
                true => (fresh_main.clone(), self.painting.clone()),
                false => (
                    of(&fresh_windows, check.window),
                    of(&self.windows.paintings, check.window),
                ),
            };
            self.verify_window(ctx, check, &fresh, &retained, everything);
        }
    }

    fn verify_window(
        &self,
        ctx: &Context,
        check: &verify::Check,
        fresh: &[(Rc<Display>, Vec2)],
        retained: &[(Rc<Display>, Vec2)],
        everything: bool,
    ) {
        let verify::Check {
            window,
            viewport,
            ref previous,
            ref region,
            moved,
        } = *check;
        assert!(
            fresh.len() == retained.len()
                && fresh
                    .iter()
                    .zip(retained)
                    .all(|((fresh, at), (retained, held))| at == held && fresh.same_tree(retained)),
            "the retained painting of {window:?} differs from painting from scratch: {:?} against {:?}",
            fresh
                .iter()
                .zip(retained)
                .find(|((fresh, at), (retained, held))| at != held || !fresh.same_tree(retained))
                .map(|((fresh, _), _)| fresh.node),
            fresh
                .iter()
                .map(|(display, at)| display.bounds.translate(*at))
                .collect::<Vec<_>>(),
        );
        let old = verify::Seen::of(previous, moved.map(|moved| moved.from));
        let new = verify::Seen::of(retained, moved.map(|moved| moved.to()));
        let changes = verify::changes(&old, &new, moved);
        if !everything && moved.is_none() && detecting_over_repaint() {
            let area = |region: &Region| {
                region
                    .rects()
                    .iter()
                    .map(|rect| rect.width() * rect.height())
                    .sum::<f32>()
            };
            let needed = verify::changed_area(&old, &new).clipped(viewport);
            let (damaged, changed_area) = (area(region), area(&needed));
            if damaged > changed_area * OVER_REPAINT_FACTOR + OVER_REPAINT_SLACK {
                let report = OverRepaint {
                    damaged: region.rects().to_vec(),
                    changed: needed.rects().to_vec(),
                };
                eprintln!("beui over-repaint: {report:?}");
                OVER_REPAINTS.with_borrow_mut(|reports| reports.push(report));
            }
        }
        let scale = ctx.window_pixels_per_point(window);
        for (rect, why) in changes.needed {
            let rect = rect.intersect(viewport);
            assert!(
                verify::covered(rect, region, scale),
                "the painting of {window:?} changed outside the damaged region: {rect:?} ({why:?}) is not within {:?}{}",
                region.rects(),
                match moved {
                    Some(moved) => format!(" (with {moved:?} copied)"),
                    None => String::new(),
                },
            );
        }
    }

    pub fn name_component(&mut self, id: NodeId, name: beui_tree::component::ComponentRecord) {
        let names = match self.component_names.remove(&id) {
            None => ComponentNames::One(name),
            Some(ComponentNames::One(first)) => ComponentNames::Many(vec![first, name]),
            Some(ComponentNames::Many(mut names)) => {
                names.push(name);
                ComponentNames::Many(names)
            }
        };
        self.component_names.insert(id, names);
    }

    pub fn component_names(&self, id: NodeId) -> Vec<&'static str> {
        self.components(id)
            .iter()
            .map(|component| component.info.name)
            .collect()
    }

    pub fn components(&self, id: NodeId) -> &[beui_tree::component::ComponentRecord] {
        match self.component_names.get(&id) {
            None => &[],
            Some(ComponentNames::One(name)) => std::slice::from_ref(name),
            Some(ComponentNames::Many(names)) => names,
        }
    }

    pub fn set_component_state_dyn(&mut self, id: NodeId, state: Box<dyn Any>) {
        self.component_states.entry(id).or_default().push(state);
    }

    pub fn api<A: Clone + 'static>(&self, id: NodeId) -> A {
        self.component_state::<A>(id).clone()
    }

    pub fn component_state<T: 'static>(&self, id: NodeId) -> &T {
        self.component_states
            .get(&id)
            .into_iter()
            .flatten()
            .rev()
            .find_map(|state| state.downcast_ref::<T>())
            .unwrap_or_else(|| panic!("component has no {} state", std::any::type_name::<T>()))
    }

    pub fn report_after_layout(&mut self, report: impl FnOnce() + 'static) {
        self.reports.push(Box::new(report));
    }

    pub fn build_within<R>(&mut self, node: NodeId, build: impl FnOnce() -> R) -> R {
        let outer = self.arena.fence(Some(node));
        let built = crate::current::lend(self, build);
        self.arena.fence(outer);
        built
    }

    pub fn invalidate_after_layout(&mut self, id: NodeId) {
        self.invalidated.push(id);
    }

    pub(crate) fn invalidated_after_layout(&self) -> usize {
        self.invalidated.len()
    }

    pub fn measured(&mut self, id: NodeId, available: Vec2) -> Option<Vec2> {
        if self.arena.stale(id) {
            self.measurements.remove(&id);
            self.baselines.remove(&id);
            self.min_widths.remove(&id);
            return None;
        }
        let held = self.measurements.get(&id)?;
        let found = held
            .iter()
            .find(|held| same_size(held.offered, available))
            .or_else(|| {
                held.iter().find(|held| {
                    !held.offer_dependent && still_fits(held.offered, held.size, available)
                })
            })
            .copied()?;
        self.offer_dependent |= found.offer_dependent;
        Some(found.size)
    }

    pub fn note_offer_dependent(&mut self) {
        self.offer_dependent = true;
    }

    pub(crate) fn enter_offer(&mut self) -> bool {
        std::mem::take(&mut self.offer_dependent)
    }

    pub(crate) fn leave_offer(&mut self, outer: bool) -> bool {
        let dependent = self.offer_dependent;
        self.offer_dependent |= outer;
        dependent
    }

    pub(crate) fn remember_measurement(
        &mut self,
        id: NodeId,
        available: Vec2,
        size: Vec2,
        offer_dependent: bool,
        watermark: u64,
    ) {
        if self.arena.layout_revision != watermark {
            return;
        }
        self.arena.clear_stale(id);
        let held = self.measurements.get_or_default(id);
        held.retain(|held| !same_size(held.offered, available));
        if held.len() >= REMEMBERED_MEASUREMENTS {
            held.remove(0);
        }
        held.push(Measurement {
            offered: available,
            size,
            offer_dependent,
        });
    }

    pub fn measured_baseline(&self, id: NodeId, available: Vec2) -> Option<Option<f32>> {
        if self.arena.stale(id) {
            return None;
        }
        self.baselines
            .get(&id)?
            .iter()
            .find(|(offered, _)| same_size(*offered, available))
            .map(|(_, baseline)| *baseline)
    }

    pub fn remember_baseline(
        &mut self,
        id: NodeId,
        available: Vec2,
        baseline: Option<f32>,
        watermark: u64,
    ) {
        if self.arena.layout_revision != watermark || self.arena.stale(id) {
            return;
        }
        let held = self.baselines.get_or_default(id);
        held.retain(|(offered, _)| !same_size(*offered, available));
        if held.len() >= REMEMBERED_MEASUREMENTS {
            held.remove(0);
        }
        held.push((available, baseline));
    }

    pub fn measured_min_width(&self, id: NodeId) -> Option<f32> {
        if self.arena.stale(id) {
            return None;
        }
        self.min_widths.get(&id).copied()
    }

    pub fn remember_min_width(&mut self, id: NodeId, width: f32, watermark: u64) {
        if self.arena.layout_revision != watermark || self.arena.stale(id) {
            return;
        }
        self.min_widths.insert(id, width);
    }

    pub fn note_measured(&self, reused: bool) {
        let counter = match reused {
            true => &self.work.reused_measurements,
            false => &self.work.measured,
        };
        counter.set(counter.get() + 1);
    }

    pub fn note_placed_work(&self, reused: bool) {
        let counter = match reused {
            true => &self.work.reused_placements,
            false => &self.work.placed,
        };
        counter.set(counter.get() + 1);
    }

    pub fn note_painted(&self, reused: bool) {
        let counter = match reused {
            true => &self.work.replayed_nodes,
            false => &self.work.painted_nodes,
        };
        counter.set(counter.get() + 1);
    }

    pub fn enter_measure(&mut self, id: NodeId) -> Option<NodeId> {
        self.layout_parent.replace(id)
    }

    pub fn leave_measure(&mut self, parent: Option<NodeId>) {
        self.layout_parent = parent;
    }

    pub fn enter_layout(&mut self, id: NodeId) -> LayoutFrame {
        let frame = LayoutFrame {
            parent: self.layout_parent,
            base: self.placing.len(),
        };
        self.layout_parent = Some(id);
        frame
    }

    pub fn leave_layout(&mut self, id: NodeId, frame: LayoutFrame, out: &Rects) {
        self.layout_parent = frame.parent;
        let mut dropped = Vec::new();
        for child in self.dropped_children(id, frame.base) {
            self.drop_placement(child, out, &mut dropped);
        }
        for node in dropped {
            self.release_placement(node);
        }
        self.placing.truncate(frame.base);
    }

    fn release_placement(&mut self, id: NodeId) {
        if !self.arena.contains(id) {
            return;
        }
        let mut element = self.arena.take(id);
        element.unplaced(self);
        self.arena.put_back(id, element);
    }

    fn dropped_children(&mut self, id: NodeId, base: usize) -> Vec<NodeId> {
        let placed = &self.placing[base..];
        let dropped: Vec<NodeId> = match self.placed_children.get(&id) {
            Some(held) if held.as_slice() == placed => return Vec::new(),
            Some(held) => {
                let kept: HashSet<NodeId> = placed.iter().copied().collect();
                held.iter()
                    .copied()
                    .filter(|child| !kept.contains(child))
                    .collect()
            }
            None => Vec::new(),
        };
        let replacement = self.placing[base..].to_vec();
        self.placed_children.insert(id, replacement);
        dropped
    }

    fn drop_placement(&mut self, id: NodeId, out: &Rects, dropped: &mut Vec<NodeId>) {
        if self.reached_pass.get(&id) == Some(&self.layout_pass) {
            return;
        }
        self.painters.remove(&id);
        if out.remove(&id).is_some() {
            self.accessibility_tree.get_mut().mark(id, &self.arena);
        }
        dropped.push(id);
        for child in self.placed_children.remove(&id).unwrap_or_default() {
            self.drop_placement(child, out, dropped);
        }
    }

    fn release_forgotten(&mut self, id: NodeId) {
        let rects = Rc::clone(&self.rects);
        let mut dropped = Vec::new();
        self.drop_placement(id, &rects, &mut dropped);
        for node in dropped {
            self.release_placement(node);
        }
    }

    fn forget_placement(&mut self, id: NodeId) {
        let rects = Rc::clone(&self.rects);
        self.drop_placement(id, &rects, &mut Vec::new());
    }

    pub fn layout_pass(&self) -> u64 {
        self.layout_pass
    }

    pub fn laying_out(&self) -> Option<NodeId> {
        self.layout_parent
    }

    pub fn report_anchor_shift(&mut self, shift: f32) {
        self.anchor_shift += shift;
    }

    pub(crate) fn replace_pinned_to_end(
        &mut self,
        pinned: Option<crate::base::list::Direction>,
    ) -> Option<crate::base::list::Direction> {
        std::mem::replace(&mut self.pinned_to_end, pinned)
    }

    pub fn pinned_to_end(&self) -> Option<crate::base::list::Direction> {
        self.pinned_to_end
    }

    pub(crate) fn take_anchor_shift(&mut self) -> f32 {
        std::mem::take(&mut self.anchor_shift)
    }

    pub(crate) fn forget_placing(&mut self) {
        self.placing.clear();
    }

    pub fn note_placed(&mut self, id: NodeId) {
        self.placing.push(id);
        self.reached_pass.insert(id, self.layout_pass);
    }

    pub fn watch_measure(
        &mut self,
        id: NodeId,
    ) -> ::reactive::ReadSignal<Option<beui_types::Measured>> {
        if let Some(watched) = self.measured.get(&id) {
            return watched.read_only();
        }
        let initial = match delaying_measurement() {
            true => {
                self.measure_pending = true;
                None
            }
            false => self.measure_now(id),
        };
        let watched = ::reactive::OwnedSignal::new(initial);
        let read = watched.read_only();
        self.measured.insert(id, watched);
        read
    }

    pub fn watch_measure_text(
        &mut self,
        id: NodeId,
    ) -> ::reactive::ReadSignal<Option<crate::base::text::TextGeometry>> {
        if let Some(watched) = self.measured_text.get(&id) {
            return watched.read_only();
        }
        let initial = match delaying_measurement() {
            true => {
                self.measure_pending = true;
                None
            }
            false => self.measure_text_now(id),
        };
        let watched = ::reactive::OwnedSignal::new(initial);
        let read = watched.read_only();
        self.measured_text.insert(id, watched);
        read
    }

    fn measure_now(&self, id: NodeId) -> Option<beui_types::Measured> {
        let placed = self.rects.placed(&id)?;
        Some(beui_types::Measured {
            rect: self.rects.transform(placed.space).rect(placed.rect),
            size: placed.rect.size(),
        })
    }

    fn measure_text_now(&self, id: NodeId) -> Option<crate::base::text::TextGeometry> {
        let text = self.arena.kind_of::<crate::base::text::TextNode>(id)?;
        self.text_geometry(text)
    }

    fn publish_measurements(&mut self) -> bool {
        type Boxes = Vec<(
            ::reactive::Signal<Option<beui_types::Measured>>,
            Option<beui_types::Measured>,
        )>;
        let moved: Boxes = self
            .measured
            .iter()
            .map(|(id, watched)| (self.measure_now(id), watched))
            .filter(|(rect, watched)| watched.get_untracked() != *rect)
            .map(|(rect, watched)| (watched.handle(), rect))
            .collect();
        type Texts = Vec<(
            ::reactive::Signal<Option<crate::base::text::TextGeometry>>,
            Option<crate::base::text::TextGeometry>,
        )>;
        let texts: Texts = self
            .measured_text
            .iter()
            .map(|(id, watched)| (self.measure_text_now(id), watched))
            .filter(|(geometry, watched)| watched.with_untracked(|held| held != geometry))
            .map(|(geometry, watched)| (watched.handle(), geometry))
            .collect();
        if moved.is_empty() && texts.is_empty() {
            return false;
        }
        crate::current::enter(self, || {
            for (write, rect) in moved {
                write.set(rect);
            }
            for (write, geometry) in texts {
                write.set(geometry);
            }
        });
        true
    }

    fn measurements_differ(&self) -> bool {
        self.measured
            .iter()
            .any(|(id, watched)| watched.get_untracked() != self.measure_now(id))
            || self.measured_text.iter().any(|(id, watched)| {
                watched.with_untracked(|held| *held != self.measure_text_now(id))
            })
    }

    pub fn reuse_placement(
        &mut self,
        id: NodeId,
        rect: Rect,
        given: PainterState,
        own: PainterState,
        out: &Rects,
    ) -> bool {
        let reusable = !self.arena.unplaced(id)
            && out.placed(&id).is_some_and(|placed| {
                placed.space == given.space && placed.rect.size() == rect.size()
            })
            && self
                .painters
                .get(&id)
                .is_some_and(|held| held.own.settles(own) && held.sight.holds(held.own, own))
            && self.placed_children.contains_key(&id);
        if !reusable {
            return false;
        }
        if let Some(held) = self.painters.get_mut(&id) {
            held.given = given;
            held.own = own;
        }
        let placed = Placed {
            rect,
            space: given.space,
        };
        let moved = out.insert(id, placed) != Some(placed);
        out.set_space(SpaceId::of(id), given.space, rect.min.to_vec2(), given.clip);
        if moved {
            self.accessibility_tree.get_mut().mark(id, &self.arena);
        }
        if moved {
            self.repaint_painter_of(id);
        }
        true
    }

    fn repaint_painter_of(&mut self, id: NodeId) {
        if let Some(parent) = self.arena.parent(id) {
            self.arena.note_relaid(parent);
        }
    }

    pub fn record_placement(
        &mut self,
        id: NodeId,
        rect: Rect,
        given: PainterState,
        own: PainterState,
        out: &Rects,
        culled: bool,
    ) {
        let placed = Placed {
            rect,
            space: given.space,
        };
        let previous = out.insert(id, placed);
        out.set_space(SpaceId::of(id), given.space, rect.min.to_vec2(), given.clip);
        self.arena.clear_unplaced(id);
        if self.arena.get(id).paints_from_layout() {
            self.arena.note_relaid(id);
        }
        self.placed_pass.insert(id, self.layout_pass);
        let held = Placing {
            given,
            own,
            sight: match culled {
                true => Sight::hidden(rect.size(), own),
                false => Sight::shown(rect.size()),
            },
            culled,
        };
        let previous_held = self
            .painters
            .insert(id, held)
            .map(|held| (held.given.clip, held.culled));
        if previous != Some(placed) || previous_held.map(|(_, was)| was) != Some(culled) {
            let parent = self.layout_parent;
            self.accessibility_tree
                .get_mut()
                .mark_placed(id, parent, &self.arena);
        }
        if previous.map(|previous| previous.rect.min) != Some(rect.min)
            || previous_held.map(|(_, was)| was) != Some(culled)
        {
            self.repaint_painter_of(id);
        }
        if previous.is_some_and(|previous| previous.rect.size() != rect.size()) {
            self.arena.note_relaid(id);
        }
        if culled {
            let mut dropped = Vec::new();
            for child in self
                .placed_children
                .insert(id, Vec::new())
                .unwrap_or_default()
            {
                self.drop_placement(child, out, &mut dropped);
            }
            if previous_held.is_none_or(|(_, was)| !was) {
                dropped.push(id);
            }
            for node in dropped {
                self.release_placement(node);
            }
        }
    }

    pub fn is_culled(&self, id: NodeId) -> bool {
        self.painters.get(&id).is_some_and(|held| held.culled)
    }

    pub fn culls(&mut self, id: NodeId, size: Vec2, own: PainterState) -> bool {
        !Sight::shows(size, own) && !self.is_overlay_open(id) && !self.holds_focus(id)
    }

    fn holds_focus(&mut self, id: NodeId) -> bool {
        let Some(focused) = self.focused else {
            return false;
        };
        let key = (self.layout_pass, focused);
        let line = &mut self.focus_line;
        if line.key != Some(key) {
            line.key = Some(key);
            line.epoch += 1;
            let mut current = Some(focused);
            for _ in 0..ANCESTOR_LIMIT {
                let Some(node) = current else { break };
                line.ancestors.insert(node, line.epoch);
                current = self.arena.parent(node);
            }
        }
        let epoch = line.epoch;
        if line.ancestors.get(&id) == Some(&epoch) {
            return true;
        }
        let mut walked = Vec::new();
        let mut current = Some(id);
        let mut within = false;
        for _ in 0..ANCESTOR_LIMIT {
            let Some(node) = current else { break };
            if let Some(&(stamp, known)) = line.within.get(&node)
                && stamp == epoch
            {
                within = known;
                break;
            }
            if node == focused {
                within = true;
                break;
            }
            walked.push(node);
            current = self.arena.parent(node);
        }
        for node in walked {
            line.within.insert(node, (epoch, within));
        }
        within
    }

    pub fn culled_ancestor(&self, id: NodeId) -> Option<NodeId> {
        let mut current = Some(id);
        for _ in 0..ANCESTOR_LIMIT {
            let node = current?;
            if self.is_culled(node) {
                return Some(node);
            }
            current = self.arena.parent(node);
        }
        None
    }

    pub fn note_space_reads(&mut self, id: NodeId, reads: bool, base: usize) {
        let Some(own) = self.painters.get(&id).map(|held| held.own) else {
            return;
        };
        let mut sight = self.painters[&id].sight.exact(reads);
        for child in self.placing.get(base..).unwrap_or_default() {
            if let Some(held) = self.painters.get(child) {
                sight = sight.and(held.sight.beneath(held.own, own));
            }
        }
        if let Some(held) = self.painters.get_mut(&id) {
            held.sight = sight;
        }
    }

    pub fn enter_space(
        &mut self,
        host: NodeId,
        slot: u8,
        painter: &Painter,
        translation: Vec2,
        clip: Rect,
        out: &Rects,
    ) -> Painter {
        self.enter_zoomed_space(host, slot, painter, translation, Zoom::NONE, clip, out)
    }

    pub fn enter_zoomed_space(
        &mut self,
        host: NodeId,
        slot: u8,
        painter: &Painter,
        translation: Vec2,
        zoom: Zoom,
        clip: Rect,
        out: &Rects,
    ) -> Painter {
        let space = SpaceId::inside(host, slot);
        let entered = painter.zoomed(space, translation, zoom, clip);
        let state = painter.state();
        let kept = state.clip.intersect(clip);
        if out.set_scaled_space(space, state.space, translation, zoom.scale, kept) {
            self.accessibility_tree.get_mut().move_space(space);
        }
        entered
    }

    fn report_laid_out(&mut self) {
        for id in std::mem::take(&mut self.invalidated) {
            if self.arena.contains(id) {
                self.arena.invalidate_node(id);
            }
        }
        self.relay_moved_overlays();
        let reports = std::mem::take(&mut self.reports);
        match delaying_measurement() {
            true => self.measure_pending |= self.measurements_differ(),
            false => {
                self.publish_measurements();
            }
        }
        if reports.is_empty() {
            return;
        }
        crate::current::enter(self, || {
            for report in reports {
                report();
            }
        });
    }

    fn log_unsettled(&self, watermark: usize) {
        if !cfg!(debug_assertions) {
            return;
        }
        let relaid: Vec<String> = self
            .arena
            .relaid_since(watermark)
            .iter()
            .filter(|id| self.arena.contains(**id))
            .take(8)
            .map(|id| {
                let names = self.component_names(*id).join(" in ");
                format!("{} {id:?} ({names})", self.node_kind(*id))
            })
            .collect();
        let message = format!(
            "beui: layout did not settle in {LAYOUTS_PER_FRAME} layouts; what each reported kept \
             changing the layout of {relaid:?}"
        );
        assert!(!failing_unsettled_layout(), "{message}");
        eprintln!("{message}");
    }

    fn run_laid_out(&mut self) {
        if let Some(laid_out) = self.laid_out.clone() {
            let context = self.reactive_scope().context();
            let _guard = crate::current::install(self);
            context.run(|| laid_out());
        }
    }

    pub fn viewport_rect(&self) -> Rect {
        self.windows
            .viewport
            .map(|(_, rect)| rect)
            .unwrap_or_else(|| {
                self.viewport
                    .as_ref()
                    .map_or(Rect::NOTHING, |(_, rect, _)| *rect)
            })
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.viewport.as_ref().map_or(1.0, |(_, _, scale)| *scale)
    }

    pub fn layout_text(&self, text: &str, font: FontId, layout: TextLayout) -> Option<Galley> {
        let (ctx, _, _) = self.viewport.as_ref()?;
        Some(ctx.painter().layout_text(text, font, layout))
    }

    pub fn rich_layout(
        &self,
        pieces: &[crate::rich::Piece],
        options: crate::rich::RichOptions,
    ) -> Option<crate::rich::RichLayout> {
        let (ctx, _, _) = self.viewport.as_ref()?;
        Some(ctx.painter().rich_layout(pieces, options))
    }

    pub fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::new(self.pixels_per_point())
    }

    pub fn measure_node(&mut self, ctx: &Context, node: NodeId, available: Vec2) -> Option<Vec2> {
        if !self.arena.contains(node) {
            return None;
        }
        let root = node;
        let painter = ctx.window_painter(self.window_of(node));
        let context = self.reactive_scope().context();
        let mut measured = None;
        {
            let _guard = crate::current::install(self);
            context.run(|| {
                crate::current::with_document(|document| {
                    measured = Some(layout::measure(document, &painter, root, available));
                });
            });
        }
        measured
    }

    fn needs_layout(&self) -> bool {
        self.layout_revision != self.arena.layout_revision
            || self.root.is_some_and(|root| self.arena.unplaced(root))
    }

    pub(crate) fn layout_frame(&self) -> u64 {
        self.layout_frame
    }

    fn update_layout(&mut self, ctx: &Context, rect: Rect) -> usize {
        self.layout_frame = self.layout_frame.wrapping_add(1);
        if std::mem::take(&mut self.measure_pending) {
            self.publish_measurements();
        }
        let passes = self.lay_out_until_settled(ctx, rect);
        if self.measure_pending {
            ctx.request_repaint();
        }
        passes
    }

    fn lay_out_until_settled(&mut self, ctx: &Context, rect: Rect) -> usize {
        let mut passes = 0;
        if self.needs_layout() {
            self.lay_out_tree(ctx, rect);
            passes += 1;
        }
        let mut watermark = self.arena.relaid_len();
        self.report_laid_out();
        self.run_laid_out();
        while self.needs_layout() {
            if passes + 1 >= LAYOUTS_PER_FRAME {
                self.log_unsettled(watermark);
                self.lay_out_tree(ctx, rect);
                ctx.request_repaint();
                return passes + 1;
            }
            self.lay_out_tree(ctx, rect);
            passes += 1;
            watermark = self.arena.relaid_len();
            self.report_laid_out();
        }
        passes
    }

    fn lay_out_tree(&mut self, ctx: &Context, rect: Rect) {
        self.layout_pass = self.layout_pass.wrapping_add(1);
        let rects = Rc::clone(&self.rects);
        self.placing.clear();
        if let Some(root) = self.root {
            let painter = ctx.window_painter(ctx.drawing_window());
            let placed = &*rects;
            self.layout_parent = None;
            crate::current::run_in(self, |document| {
                layout::layout(document, &painter, root, rect, placed);
                document.lay_out_boundaries(ctx, placed);
                document.place_moved_overlays(ctx, placed);
            });
        }
        self.placing.clear();
        self.canvas_moves_from += self.canvas_moves.len() as u64;
        self.canvas_moves.clear();
        self.anchor_shift = 0.0;
        self.layout_revision = self.arena.layout_revision;
        self.layout_pass = self.layout_pass.wrapping_add(1);
    }
}

const OVER_REPAINT_FACTOR: f32 = 4.0;
const OVER_REPAINT_SLACK: f32 = 64.0 * 64.0;

#[derive(Clone, Debug, PartialEq)]
pub struct OverRepaint {
    pub damaged: Vec<Rect>,
    pub changed: Vec<Rect>,
}

thread_local! {
    static OVER_REPAINTS: RefCell<Vec<OverRepaint>> = const { RefCell::new(Vec::new()) };
    static DETECT_OVER_REPAINT: Cell<bool> =
        Cell::new(std::env::var_os("BEUI_OVER_REPAINT").is_some());
}

pub fn detect_over_repaint(enabled: bool) {
    DETECT_OVER_REPAINT.set(enabled);
}

fn detecting_over_repaint() -> bool {
    DETECT_OVER_REPAINT.get()
}

pub fn take_over_repaints() -> Vec<OverRepaint> {
    OVER_REPAINTS.with_borrow_mut(std::mem::take)
}

static VERIFY_PAINT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn verify_paint(enabled: bool) {
    VERIFY_PAINT.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

fn verifying_paint() -> bool {
    cfg!(test) || VERIFY_PAINT.load(std::sync::atomic::Ordering::Relaxed)
}

static FAIL_UNSETTLED_LAYOUT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn fail_unsettled_layout(enabled: bool) {
    FAIL_UNSETTLED_LAYOUT.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

thread_local! {
    static DELAY_MEASUREMENT: Cell<bool> =
        Cell::new(std::env::var_os("BEUI_DELAY_MEASUREMENT").is_some_and(|value| value == "1"));
}

pub fn delay_measurement(enabled: bool) {
    DELAY_MEASUREMENT.set(enabled);
}

fn delaying_measurement() -> bool {
    DELAY_MEASUREMENT.get()
}

fn failing_unsettled_layout() -> bool {
    cfg!(test) || FAIL_UNSETTLED_LAYOUT.load(std::sync::atomic::Ordering::Relaxed)
}

const LAYOUTS_PER_FRAME: usize = 8;

impl Document {
    fn lay_out_boundaries(&mut self, ctx: &Context, rects: &Rects) {
        for boundary in self.arena.take_boundaries() {
            if !self.arena.contains(boundary)
                || !self.arena.unplaced(boundary)
                || self.reached_pass.get(&boundary) == Some(&self.layout_pass)
            {
                continue;
            }
            let (Some(placed), Some(held)) =
                (rects.placed(&boundary), self.painters.get(&boundary))
            else {
                continue;
            };
            let transform = rects.transform(held.given.space);
            let state = PainterState {
                origin: transform.offset,
                scale: transform.scale,
                space_clip: rects.space_clip(held.given.space),
                ..held.given
            };
            let painter = Painter::resumed(ctx.clone(), state);
            self.layout_parent = self.arena.parent(boundary);
            let viewport = self.window_viewport(self.window_of(boundary));
            let held = self.swap_window_viewport(viewport);
            layout::layout(self, &painter, boundary, placed.rect, rects);
            self.swap_window_viewport(held);
            self.layout_parent = None;
            self.placing.clear();
        }
    }
}

pub fn trim_bottom(rect: Rect, height: f32) -> (Rect, Rect) {
    let height = height.clamp(0.0, rect.height().max(0.0));
    let edge = rect.bottom() - height;
    (
        Rect::from_min_max(rect.min, pos2(rect.right(), edge)),
        Rect::from_min_max(pos2(rect.left(), edge), rect.max),
    )
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

enum ComponentNames {
    One(beui_tree::component::ComponentRecord),
    Many(Vec<beui_tree::component::ComponentRecord>),
}
