use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::time::Instant;

use accesskit::Node;

use crate::accessibility::{self, AccessibilityTree};
use crate::base::child_list::{ChildHost, NodeChildren, SlotId};
use crate::base::fade::FadeNode;
use crate::base::frame::Sides;
use crate::context::{Context, Moved};
use crate::damage::{Damage, Region};
use crate::file_picker::{FileFilter, FilePick, FilePickId};
use crate::flash::FlashLog;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::input::{Key, KeyChord, KeyPress, Modifiers};

use crate::display::Display;
use crate::interact::{self, Keys};
use crate::layout;
use crate::node::{ANCESTOR_LIMIT, Arena, NodeId, NodeMap, NodeOf, Placed, Rects, SpaceId};
use crate::paint::{self, PaintCache};
use crate::painter::{Entry, Painter, PainterState, Shape};
use crate::performance::{FrameMeasurement, FrameWork, PerformanceSnapshot, PerformanceTracker};
use crate::pixel_grid::PixelGrid;
use crate::screen_simulation::{self, Placement};
use crate::screens::Screen;
use crate::sight::Sight;

pub type Shortcut = dyn Fn(KeyPress) -> bool;
type PickedCallback = Box<dyn FnOnce(FilePick)>;
pub type FingerTap = dyn Fn(usize) -> bool;
pub type UnhandledKey = dyn Fn(UnhandledKeyPress) -> bool;
pub type GlobalKey = dyn Fn(GlobalKeyPress) -> bool;

#[derive(Clone, Copy, Debug)]
pub struct GlobalKeyPress {
    pub press: KeyPress,
    pub typing: bool,
    pub in_app: bool,
    pub held: bool,
    pub tap: bool,
}

#[derive(Clone, Debug)]
pub struct UnhandledKeyPress {
    pub press: KeyPress,
    pub focus_path: Vec<NodeId>,
    pub typing: bool,
}

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
    pub focused: Option<NodeId>,
    pub composed: String,
    pub(crate) keyboard_held: bool,
    pub activated: Option<NodeId>,
    pub activation_key: Option<Key>,
    pub rects: Rc<Rects>,
    tools: Option<Box<dyn Tools>>,
    inspector_requested: bool,
    screen_pointer: Option<Pos2>,
    pub last_pointer: Option<crate::input::PointerSample>,
    pub acted: Acted,
    placement: Option<(Rect, Option<Placement>)>,
    pub portal_holders: std::collections::HashMap<NodeId, NodeOf<crate::base::portal::PortalNode>>,
    pub overlay_stack: Vec<NodeOf<crate::base::overlay::OverlayNode>>,
    pub passive_overlays: Vec<NodeOf<crate::base::overlay::OverlayNode>>,
    pub back_handlers: Vec<NodeOf<crate::base::back::BackNode>>,
    pub back_gesture: Option<NodeId>,
    timers: RefCell<crate::timer::Timers>,
    scale: (::reactive::ReadSignal<f32>, ::reactive::WriteSignal<f32>),
    attached: (::reactive::ReadSignal<u64>, ::reactive::WriteSignal<u64>),
    focus_visible: (::reactive::ReadSignal<bool>, ::reactive::WriteSignal<bool>),
    screens: (
        ::reactive::ReadSignal<Vec<Screen>>,
        ::reactive::WriteSignal<Vec<Screen>>,
    ),
    offered_screens: Option<Vec<Screen>>,
    rescreened: RefCell<Option<Vec<Screen>>>,
    reattached: Cell<bool>,
    shortcuts: RefCell<Vec<Weak<Shortcut>>>,
    finger_taps: RefCell<Vec<Weak<FingerTap>>>,
    unhandled_keys: RefCell<Vec<Weak<UnhandledKey>>>,
    global_keys: RefCell<Vec<Weak<GlobalKey>>>,
    pub(crate) globally_held: Vec<(Key, Option<Rc<GlobalKey>>)>,
    pub(crate) tapping: Option<Key>,
    input_frames: u64,
    modifiers: (
        ::reactive::ReadSignal<Modifiers>,
        ::reactive::WriteSignal<Modifiers>,
    ),
    intercepted_keys: (
        ::reactive::ReadSignal<Vec<KeyChord>>,
        ::reactive::WriteSignal<Vec<KeyChord>>,
    ),
    pub touch_scroll_vertical: Option<NodeId>,
    pub touch_shift: crate::geometry::Vec2,
    pub touch_scroll_horizontal: Option<NodeId>,
    pub wheel_latch: Option<(NodeId, Instant, Option<crate::geometry::Pos2>)>,
    pub autoscroll: Option<crate::interact::autoscroll::Autoscroll>,
    pub pointer_capture: Option<NodeId>,
    pub press_claim: Option<NodeId>,
    pub secondary_claim: Option<NodeId>,
    pub(crate) press_claimants: HashSet<NodeId>,
    pub(crate) outside_watchers: HashSet<NodeId>,
    pub forward: crate::interact::forward::Routing,
    pub drags: Rc<crate::drag_board::Board>,
    paste_requested: bool,
    unsent_file_picks: Vec<(FileFilter, PickedCallback)>,
    waiting_file_picks: Vec<(FilePickId, PickedCallback)>,
    test_ids: HashMap<String, Vec<NodeId>>,
    node_test_ids: HashMap<NodeId, Vec<String>>,
    layout_revision: u64,
    paint_revision: u64,
    delivering: bool,
    pub deferred_reveals: Vec<NodeId>,
    pub deferred_fades: Vec<(NodeOf<FadeNode>, Sides)>,
    interaction_done: Option<Rc<dyn Fn()>>,
    laid_out: Option<Rc<dyn Fn()>>,
    constrained: HashSet<NodeId>,
    measurements: NodeMap<Vec<(Vec2, Vec2)>>,
    baselines: NodeMap<Vec<(Vec2, Option<f32>)>>,
    layout_parent: Option<NodeId>,
    placed_children: NodeMap<Vec<NodeId>>,
    placing: Vec<NodeId>,
    interact_pool: Vec<Vec<NodeId>>,
    pub engaged: Vec<NodeId>,
    pub interact_parents: NodeMap<NodeId>,
    pub interacted: NodeMap<u64>,
    pub interact_pass: u64,
    pub interact_bounds: NodeMap<Rect>,
    pub interact_bounds_version: Option<u64>,
    pub placed_pass: NodeMap<u64>,
    pub reached_pass: NodeMap<u64>,
    layout_pass: u64,
    scroll_hosts: Vec<NodeId>,
    scroll_shifts: NodeMap<f32>,
    viewport: Option<(Context, Rect, f32)>,
    fonts_generation: u64,
    painting: Vec<(Rc<Display>, Entry)>,
    pub paint_cache: RefCell<PaintCache>,
    pub verifies_paint: bool,
    pub copied_text: Option<String>,
    next_paint: Option<Instant>,
    now: Instant,
    reactive_scope: ::reactive::Scope,
    zone: u64,
    extensions: HashMap<std::any::TypeId, Box<dyn Any>>,
    node_scopes: HashMap<NodeId, Vec<::reactive::Scope>>,
    node_refs: HashMap<NodeId, Vec<Weak<Cell<Option<NodeId>>>>>,
    sizes: NodeMap<Vec<SizeWatcher>>,
    placements: NodeMap<Vec<PlacementWatcher>>,
    placed: NodeMap<(::reactive::ReadSignal<bool>, ::reactive::WriteSignal<bool>)>,
    component_states: HashMap<NodeId, Vec<Box<dyn Any>>>,
    component_names: HashMap<NodeId, Vec<&'static str>>,
    pub accessibility_id: u32,
    pub accessibility: NodeMap<Node>,
    pub accessibility_tree: RefCell<AccessibilityTree>,
    performance: PerformanceTracker,
    pub work: WorkCounters,
    changes: FlashLog<NodeId>,
    damage: Damage,
    damage_flashes: FlashLog<Rect>,
    painters: NodeMap<Placing>,
    spaces_moved: bool,
    rubber_banding: bool,
}

#[derive(Clone, Copy)]
struct Placing {
    given: PainterState,
    own: PainterState,
    sight: Sight,
    culled: bool,
}

struct SizeWatcher {
    read: ::reactive::ReadSignal<Vec2>,
    write: ::reactive::WriteSignal<Vec2>,
}

const REMEMBERED_MEASUREMENTS: usize = 4;

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

fn constrained(held: Vec2, available: Vec2) -> Vec2 {
    vec2(
        match available.x.is_finite() {
            true => available.x,
            false => held.x,
        },
        match available.y.is_finite() {
            true => available.y,
            false => held.y,
        },
    )
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
    }

    pub fn note_described(&self, nodes: usize) {
        self.described_nodes.set(self.described_nodes.get() + nodes);
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

struct PlacementWatcher {
    read: ::reactive::ReadSignal<Rect>,
    write: ::reactive::WriteSignal<Rect>,
}

static NEXT_ZONE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Document {
    pub fn new() -> Self {
        Self {
            arena: Arena::default(),
            root: None,
            focused: None,
            composed: String::new(),
            keyboard_held: false,
            activated: None,
            activation_key: None,
            rects: Rc::new(Rects::default()),
            tools: None,
            inspector_requested: false,
            screen_pointer: None,
            last_pointer: None,
            acted: Acted::Nothing,
            placement: None,
            portal_holders: std::collections::HashMap::new(),
            overlay_stack: Vec::new(),
            passive_overlays: Vec::new(),
            back_handlers: Vec::new(),
            back_gesture: None,
            timers: RefCell::new(Vec::new()),
            scale: ::reactive::create_signal(1.0),
            attached: ::reactive::create_signal(0),
            focus_visible: ::reactive::create_signal(false),
            screens: ::reactive::create_signal(Vec::new()),
            offered_screens: None,
            rescreened: RefCell::new(None),
            reattached: Cell::new(false),
            shortcuts: RefCell::new(Vec::new()),
            finger_taps: RefCell::new(Vec::new()),
            unhandled_keys: RefCell::new(Vec::new()),
            global_keys: RefCell::new(Vec::new()),
            globally_held: Vec::new(),
            tapping: None,
            input_frames: 0,
            modifiers: ::reactive::create_signal(Modifiers::NONE),
            intercepted_keys: ::reactive::create_signal(Vec::new()),
            touch_scroll_vertical: None,
            touch_shift: crate::geometry::Vec2::ZERO,
            touch_scroll_horizontal: None,
            wheel_latch: None,
            autoscroll: None,
            pointer_capture: None,
            press_claim: None,
            secondary_claim: None,
            press_claimants: HashSet::new(),
            outside_watchers: HashSet::new(),
            forward: Default::default(),
            drags: Rc::default(),
            paste_requested: false,
            unsent_file_picks: Vec::new(),
            waiting_file_picks: Vec::new(),
            test_ids: HashMap::new(),
            node_test_ids: HashMap::new(),
            layout_revision: 0,
            paint_revision: 0,
            delivering: false,
            deferred_reveals: Vec::new(),
            deferred_fades: Vec::new(),
            interaction_done: None,
            laid_out: None,
            constrained: HashSet::new(),
            measurements: NodeMap::default(),
            baselines: NodeMap::default(),
            layout_parent: None,
            placed_children: NodeMap::default(),
            placing: Vec::new(),
            interact_pool: Vec::new(),
            engaged: Vec::new(),
            interact_parents: NodeMap::default(),
            interacted: NodeMap::default(),
            interact_pass: 0,
            interact_bounds: NodeMap::default(),
            interact_bounds_version: None,
            placed_pass: NodeMap::default(),
            reached_pass: NodeMap::default(),
            layout_pass: 0,
            scroll_hosts: Vec::new(),
            scroll_shifts: NodeMap::default(),
            viewport: None,
            fonts_generation: 0,
            painting: Vec::new(),
            paint_cache: RefCell::new(PaintCache::default()),
            verifies_paint: true,
            copied_text: None,
            next_paint: None,
            now: Instant::now(),
            reactive_scope: ::reactive::Scope::new(),
            zone: NEXT_ZONE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            extensions: HashMap::new(),
            node_scopes: HashMap::new(),
            node_refs: HashMap::new(),
            sizes: NodeMap::default(),
            placements: NodeMap::default(),
            placed: NodeMap::default(),
            component_states: HashMap::new(),
            component_names: HashMap::new(),
            accessibility_id: accessibility::next_document_id(),
            accessibility: NodeMap::default(),
            accessibility_tree: RefCell::default(),
            performance: PerformanceTracker::default(),
            work: WorkCounters::default(),
            changes: FlashLog::default(),
            damage: Damage::default(),
            damage_flashes: FlashLog::default(),
            painters: NodeMap::default(),
            spaces_moved: false,
            rubber_banding: true,
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

    pub fn register_timer(&self, timer: Weak<crate::timer::TimerState>) {
        self.timers.borrow_mut().push(timer);
    }

    pub fn watch_context(&self) -> ::reactive::ReadSignal<u64> {
        self.attached.0.clone()
    }

    pub fn watch_pixels_per_point(&self) -> ::reactive::ReadSignal<f32> {
        self.scale.0.clone()
    }

    pub fn watch_screens(&self) -> ::reactive::ReadSignal<Vec<Screen>> {
        self.screens.0.clone()
    }

    pub fn screens(&self) -> Vec<Screen> {
        self.screens.0.get_untracked()
    }

    pub fn screen_at(&self, pos: Pos2) -> Option<Screen> {
        self.screens
            .0
            .with_untracked(|screens| crate::screens::at(screens, pos).cloned())
    }

    pub fn screen_under(&self, rect: Rect) -> Option<Screen> {
        self.screens
            .0
            .with_untracked(|screens| crate::screens::under(screens, rect).cloned())
    }

    pub fn screen_named(&self, id: &str) -> Option<Screen> {
        self.screens
            .0
            .with_untracked(|screens| screens.iter().find(|screen| screen.id == id).cloned())
    }

    pub fn watch_focus_visible(&self) -> ::reactive::ReadSignal<bool> {
        self.focus_visible.0.clone()
    }

    pub fn set_focus_visible(&self, visible: bool) {
        self.focus_visible.1.set(visible);
    }

    pub fn register_shortcut(&self, shortcut: Weak<Shortcut>) {
        self.shortcuts.borrow_mut().push(shortcut);
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

    pub fn register_unhandled_key(&self, handler: Weak<UnhandledKey>) {
        self.unhandled_keys.borrow_mut().push(handler);
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

    pub fn key_unhandled(&self, press: KeyPress) -> bool {
        let mut handlers = self.unhandled_keys.borrow_mut();
        handlers.retain(|handler| handler.strong_count() > 0);
        let live: Vec<Rc<UnhandledKey>> = handlers.iter().filter_map(Weak::upgrade).collect();
        drop(handlers);
        if live.is_empty() {
            return false;
        }
        let unhandled = UnhandledKeyPress {
            press,
            focus_path: self.focus_ancestry(),
            typing: self.focus_types(),
        };
        live.into_iter().any(|handler| handler(unhandled.clone()))
    }

    pub fn register_global_key(&self, handler: Weak<GlobalKey>) {
        self.global_keys.borrow_mut().push(handler);
    }

    pub fn key_global(&self, global: GlobalKeyPress) -> bool {
        self.global_taker(global).is_some()
    }

    pub(crate) fn global_taker(&self, global: GlobalKeyPress) -> Option<Rc<GlobalKey>> {
        if self.locked() && !global.press.key.is_media() {
            return None;
        }
        let mut handlers = self.global_keys.borrow_mut();
        handlers.retain(|handler| handler.strong_count() > 0);
        let live: Vec<Rc<GlobalKey>> = handlers.iter().filter_map(Weak::upgrade).collect();
        drop(handlers);
        live.into_iter().find(|handler| handler(global))
    }

    pub fn input_frames(&self) -> u64 {
        self.input_frames
    }

    pub(crate) fn note_input(&mut self) {
        self.input_frames += 1;
    }

    pub fn watch_modifiers(&self) -> ::reactive::ReadSignal<Modifiers> {
        self.modifiers.0.clone()
    }

    pub fn intercepted_keys_writer(&self) -> ::reactive::WriteSignal<Vec<KeyChord>> {
        self.intercepted_keys.1.clone()
    }

    pub fn intercepted_keys(&self) -> Vec<KeyChord> {
        self.intercepted_keys.0.get_untracked()
    }

    pub fn set_modifiers(&self, modifiers: Modifiers) {
        if self.modifiers.0.get_untracked() != modifiers {
            self.modifiers.1.set(modifiers);
        }
    }

    pub fn key_shortcut(&self, press: KeyPress) -> bool {
        let mut shortcuts = self.shortcuts.borrow_mut();
        shortcuts.retain(|shortcut| shortcut.strong_count() > 0);
        let live: Vec<Rc<Shortcut>> = shortcuts.iter().filter_map(Weak::upgrade).collect();
        drop(shortcuts);
        live.into_iter().any(|shortcut| shortcut(press))
    }

    fn run_timers(&self) {
        let scale = self.pixels_per_point();
        if self.scale.0.get_untracked() != scale {
            self.scale.1.set(scale);
        }
        let rescreened = self.rescreened.borrow_mut().take();
        if let Some(screens) = rescreened {
            self.screens.1.set(screens);
        }
        if self.reattached.replace(false) {
            self.attached.1.update(|attached| *attached += 1);
        }
        let now = self.now;
        let mut timers = self.timers.borrow_mut();
        timers.retain(|timer| timer.strong_count() > 0);
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

    pub fn register_node_scope(&mut self, node: NodeId, scope: ::reactive::Scope) {
        self.node_scopes.entry(node).or_default().push(scope);
    }

    pub fn register_node_ref(&mut self, node: NodeId, cell: Weak<Cell<Option<NodeId>>>) {
        let refs = self.node_refs.entry(node).or_default();
        refs.retain(|cell| cell.strong_count() > 0);
        refs.push(cell);
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.arena.get(id).children()
    }

    pub fn overlay_layers(&self) -> Vec<(Vec<usize>, Rect)> {
        self.overlays_bottom_up()
            .into_iter()
            .enumerate()
            .filter_map(|(index, overlay)| Some((vec![index + 1], self.overlay_occluder(overlay)?)))
            .collect()
    }

    pub fn paint_order(&self, node: NodeId) -> Vec<usize> {
        let overlays = self.overlays_bottom_up();
        let mut path = Vec::new();
        let mut current = node;
        let layer = loop {
            if let Some(layer) = overlays.iter().position(|overlay| overlay.id() == current) {
                break layer + 1;
            }
            let Some(parent) = self.arena.parent(current) else {
                break 0;
            };
            let index = self
                .children(parent)
                .iter()
                .position(|child| *child == current)
                .unwrap_or_default();
            path.push(index);
            current = parent;
        };
        path.push(layer);
        path.reverse();
        path
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
        self.arena.get_mut_as::<H>(node).children().push(item);
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
        }
    }

    pub fn clear_test_id(&mut self, id: NodeId, test_id: &str) {
        self.forget_test_id(id, test_id);
        self.drop_test_id(id, test_id);
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
        let mut live = HashSet::new();
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

    pub fn find_test_id(&self, test_id: &str) -> Option<NodeId> {
        let named = self.named_live(test_id, &self.live_nodes());
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

    pub fn rubber_banding(&self) -> bool {
        self.rubber_banding
    }

    pub fn set_rubber_banding(&mut self, enabled: bool) {
        self.rubber_banding = enabled;
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
            self.arena.invalidate_node(id);
        }
    }

    pub fn copy_text(&mut self, text: impl Into<String>) {
        self.copied_text = Some(text.into());
    }

    pub fn request_paste(&mut self) {
        self.paste_requested = true;
    }

    pub fn pick_file(&mut self, filter: FileFilter, picked: impl FnOnce(FilePick) + 'static) {
        self.unsent_file_picks.push((filter, Box::new(picked)));
    }

    fn send_file_picks(&mut self, ctx: &Context) {
        for (filter, picked) in std::mem::take(&mut self.unsent_file_picks) {
            let id = ctx.pick_file(filter);
            self.waiting_file_picks.push((id, picked));
        }
    }

    fn deliver_file_picks(&mut self, ctx: &Context) {
        if self.waiting_file_picks.is_empty() {
            return;
        }
        let mut delivered = Vec::new();
        for (id, picked) in std::mem::take(&mut self.waiting_file_picks) {
            match ctx.take_file_pick(id) {
                Some(pick) => delivered.push((picked, pick)),
                None => self.waiting_file_picks.push((id, picked)),
            }
        }
        if delivered.is_empty() {
            return;
        }
        let context = self.reactive_scope().context();
        let _guard = crate::current::install(self);
        context.run(|| {
            for (picked, pick) in delivered {
                picked(pick);
            }
        });
    }

    pub fn remove_node(&mut self, id: NodeId) {
        let mut dropped = Vec::new();
        if !self.delivering {
            let rects = Rc::clone(&self.rects);
            self.drop_placement(id, &rects, &mut dropped);
        }
        let mut scopes = Vec::new();
        self.detach_subtree(id, &mut scopes);
        drop(scopes);
        for node in dropped {
            self.release_placement(node);
        }
    }

    fn detach_subtree(&mut self, id: NodeId, scopes: &mut Vec<::reactive::Scope>) {
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
            if borrowed.contains(&child) {
                continue;
            }
            self.detach_subtree(child, scopes);
        }
        let mut element = self.arena.take(id);
        element.detached();
        self.arena.put_back(id, element);
        self.arena.remove(id);
        self.overlay_stack.retain(|overlay| overlay.id() != id);
        self.passive_overlays.retain(|overlay| overlay.id() != id);
        self.back_handlers.retain(|handler| handler.id() != id);
        self.paint_cache.borrow_mut().forget(id);
        self.sizes.remove(&id);
        self.placements.remove(&id);
        self.placed.remove(&id);
        self.measurements.remove(&id);
        self.baselines.remove(&id);
        self.component_states.remove(&id);
        self.component_names.remove(&id);
        self.placed_children.remove(&id);
        self.placed_pass.remove(&id);
        self.press_claimants.remove(&id);
        self.outside_watchers.remove(&id);
        self.reached_pass.remove(&id);
        self.scroll_shifts.remove(&id);
        self.accessibility.remove(&id);
        self.accessibility_tree.get_mut().forget(id, &self.arena);
        for test_id in self.node_test_ids.remove(&id).unwrap_or_default() {
            self.drop_test_id(id, &test_id);
        }
        scopes.extend(self.node_scopes.remove(&id).unwrap_or_default());
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
            self.focused = None;
        }
        if self.activated == Some(id) {
            self.activated = None;
        }
    }

    pub fn show(&mut self, ctx: &Context, rect: Rect) {
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
        self.show_screen(ctx, content, true, Keys::All);
        ctx.show_mouse_simulation(rect);
    }

    pub fn on_interacted(&mut self, interacted: impl Fn() + 'static) {
        self.interaction_done = Some(Rc::new(interacted));
    }

    pub fn on_laid_out(&mut self, laid_out: impl Fn() + 'static) {
        self.laid_out = Some(Rc::new(laid_out));
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
        self.viewport = None;
    }

    pub fn show_screen(&mut self, ctx: &Context, rect: Rect, pointer: bool, keys: Keys) {
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
        self.offered_screens = match placement {
            Some(placement) => Some(vec![Screen::window(placement.screen)]),
            None => Some(crate::screens::shown_in(&ctx.screens(), rect)),
        };
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
                        self.show_content(ctx, placement.screen, pointer, keys);
                    });
                });
            }
            None => self.show_content(ctx, rect, pointer, keys),
        }
    }

    pub fn screen_scale(&self) -> f32 {
        self.shown_screen().map_or(1.0, |placement| placement.scale)
    }

    pub fn shown_screen(&self) -> Option<Placement> {
        self.placement.and_then(|(_, placement)| placement)
    }

    pub fn now(&self) -> Instant {
        self.now
    }

    pub fn show_content(&mut self, ctx: &Context, rect: Rect, pointer: bool, keys: Keys) {
        self.now = ctx.now();
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
        let screens = self
            .offered_screens
            .take()
            .unwrap_or_else(|| vec![Screen::window(rect)]);
        let rescreened = self.screens.0.with_untracked(|shown| *shown != screens);
        if rescreened {
            self.arena.invalidate();
            *self.rescreened.borrow_mut() = Some(screens);
        }
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
            self.arena.invalidate();
            self.viewport = Some((ctx.clone(), rect, scale));
        }
        FrameMeasurement::measure(&mut measurement.timings.accessibility, || {
            let actions = ctx.take_accessibility_actions(self.accessibility_id);
            if !actions.is_empty() {
                let context = self.reactive_scope().context();
                let _guard = crate::current::install(self);
                context.run(|| {
                    crate::current::with_document(|document| {
                        for request in actions {
                            document.handle_accessibility_action(request);
                        }
                    });
                });
            }
        });

        {
            let context = self.reactive_scope().context();
            let _guard = crate::current::install(self);
            context.run(|| crate::current::with_document(|document| document.run_timers()));
        }
        self.deliver_file_picks(ctx);

        if pointer || !keys.ignored() {
            FrameMeasurement::measure(&mut measurement.timings.interaction, || {
                if let Some(root) = self.root {
                    let rects = Rc::clone(&self.rects);
                    let painter = ctx.painter();
                    let context = self.reactive_scope().context();
                    let _guard = crate::current::install(self);
                    context.run(|| {
                        crate::current::with_document(|document| {
                            interact::interact(document, ctx, &painter, &rects, root, pointer, keys)
                        });
                    });
                }
            });
        }
        if let Some(interacted) = self.interaction_done.clone() {
            let context = self.reactive_scope().context();
            let _guard = crate::current::install(self);
            context.run(|| interacted());
        }
        if self.handles_back() {
            ctx.handle_back();
        }
        if self.modal_open() {
            ctx.want_keyboard();
        }
        self.intercepted_keys
            .0
            .with_untracked(|chords| ctx.intercept_keys(chords));

        if let Some(text) = self.copied_text.take() {
            ctx.copy_text(text);
        }
        if std::mem::take(&mut self.paste_requested) {
            ctx.request_paste();
        }
        measurement.layout_passes =
            FrameMeasurement::measure(&mut measurement.timings.layout, || {
                let mut passes = usize::from(self.update_layout(ctx, rect));
                if resized && self.focus_takes_text() {
                    self.reveal_focus(&ctx.painter());
                    passes += usize::from(self.update_layout(ctx, rect));
                }
                passes
            });
        if let Some(laid_out) = self.laid_out.clone() {
            for _ in 0..LAID_OUT_ROUNDS {
                let context = self.reactive_scope().context();
                {
                    let _guard = crate::current::install(self);
                    context.run(|| laid_out());
                }
                let moved = FrameMeasurement::measure(&mut measurement.timings.layout, || {
                    self.update_layout(ctx, rect)
                });
                measurement.layout_passes += usize::from(moved);
                if !moved {
                    break;
                }
            }
        }
        if !keys.ignored()
            && let Some(area) = self.focused_ime_area()
        {
            ctx.set_ime_area(Some(area));
        }
        if ctx.test_ids_published() {
            let mut live = None;
            for (test_id, nodes) in &self.test_ids {
                let shown = match nodes.as_slice() {
                    [node] => Some(*node),
                    _ => {
                        let live = live.get_or_insert_with(|| self.live_nodes());
                        match self.named_live(test_id, live).as_slice() {
                            [] => None,
                            [node] => Some(*node),
                            _ => {
                                ctx.publish_ambiguous_test_id(test_id);
                                None
                            }
                        }
                    }
                };
                if let Some(node_rect) = shown.and_then(|id| self.rects.get(&id)) {
                    ctx.publish_test_id(test_id, node_rect);
                }
            }
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
        if !repaints.is_empty() || self.paint_revision != self.arena.revision {
            measurement.painted = true;
            let verifying = self.verifies_paint && verifying_paint();
            let previous = verifying.then(|| flatten(&self.painting));
            FrameMeasurement::measure(&mut measurement.timings.paint, || self.paint(ctx));
            self.paint_revision = self.arena.revision;
            let moves = self.paint_cache.get_mut().take_moves();
            let everything = self.damage.is_everything();
            let region = self.damage.take(rect);
            let (region, moved) = self.settle_moves(moves, region, rect);
            if let Some(previous) = previous {
                self.verify_paint(ctx, rect, &previous, &region, moved, everything);
            }
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
        ctx.show_painting(&self.painting, rect);
        self.send_file_picks(ctx);
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
        let painter = ctx.painter();
        let mut roots = Vec::new();
        if let Some(root) = self.root {
            paint::paint(self, &painter, &self.rects, root);
            roots.push(root);
        }
        for overlay in self.overlays_bottom_up() {
            let scrim = self.overlay_scrim(overlay);
            let content = self.overlay_content(overlay);
            for layer in [Some(scrim), content].into_iter().flatten() {
                if self.rects.contains_key(&layer) {
                    paint::paint(self, &painter, &self.rects, layer);
                    roots.push(layer);
                }
            }
        }
        let cache = self.paint_cache.get_mut();
        cache.settle_roots(roots);
        if cache.take_recorded() {
            self.painting = cache.painting();
        }
        self.damage.add_region(cache.take_damage());
    }

    fn settle_moves(
        &self,
        moves: Vec<paint::Move>,
        region: Region,
        viewport: Rect,
    ) -> (Region, Option<Moved>) {
        let shown_area = |moved: &paint::Move| {
            let visible = moved.viewport.intersect(viewport);
            visible.width().max(0.0) * visible.height().max(0.0)
        };
        let Some(largest) = (0..moves.len())
            .max_by(|a, b| shown_area(&moves[*a]).total_cmp(&shown_area(&moves[*b])))
        else {
            return (region, None);
        };
        let mut region = region;
        for (index, moved) in moves.iter().enumerate() {
            if index != largest {
                region.add(moved.viewport.intersect(viewport));
            }
        }
        let only = &moves[largest];
        let visible = only.viewport.intersect(viewport);
        let rooted = self.paint_cache.borrow().rooted();
        let (fixed, inner) =
            paint::fixed_damage(&rooted, only.node, &only.moving, visible, only.by);
        let shown = visible.intersect(inner);
        let landed = shown.intersect(shown.translate(only.by));
        let damaged = region
            .rects()
            .iter()
            .filter(|rect| {
                !only
                    .absorbed
                    .iter()
                    .any(|absorbed| absorbed.contains_rect(**rect))
            })
            .flat_map(|rect| [*rect, rect.translate(only.by).intersect(landed)])
            .chain(only.damaged.clipped(viewport).rects().iter().copied())
            .chain(fixed.rects().iter().copied())
            .collect::<Vec<Rect>>();
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
            from: landed.translate(-only.by),
            by: only.by,
        };
        (settled, Some(moved))
    }

    fn verify_paint(
        &mut self,
        ctx: &Context,
        viewport: Rect,
        previous: &[Shape],
        region: &crate::damage::Region,
        moved: Option<Moved>,
        everything: bool,
    ) {
        let counted = (
            self.work.painted_nodes.get(),
            self.work.replayed_nodes.get(),
        );
        let retained = self.paint_cache.replace(PaintCache::default());
        let painting = std::mem::take(&mut self.painting);
        let damage = std::mem::take(&mut self.damage);
        self.paint(ctx);
        let fresh = flatten(&std::mem::replace(&mut self.painting, painting));
        self.paint_cache.replace(retained);
        self.damage = damage;
        self.work.painted_nodes.set(counted.0);
        self.work.replayed_nodes.set(counted.1);
        let shapes = flatten(&self.painting);
        let differs = fresh
            .iter()
            .zip(&shapes)
            .position(|(fresh, retained)| !paint::same_shape(fresh, retained));
        assert!(
            fresh.len() == shapes.len() && differs.is_none(),
            "the retained painting differs from painting from scratch: {} shapes against {}, first at {differs:?}: {:?} against {:?}",
            fresh.len(),
            shapes.len(),
            differs.map(|at| crate::damage::bounds(&fresh[at])),
            differs.map(|at| crate::damage::bounds(&shapes[at])),
        );
        let kept =
            |old: &Shape, new: &Shape| old == new || paint::redrawn_in_place(old, new).is_some();
        let prefix = previous
            .iter()
            .zip(&shapes)
            .take_while(|(old, new)| kept(old, new))
            .count();
        let suffix = previous[prefix..]
            .iter()
            .rev()
            .zip(shapes[prefix..].iter().rev())
            .take_while(|(old, new)| kept(old, new))
            .count();
        let old = &previous[prefix..previous.len() - suffix];
        let new = &shapes[prefix..shapes.len() - suffix];
        let landed = moved.map_or(Rect::NOTHING, |moved| moved.from.translate(moved.by));
        let changed: Vec<&Shape> = old
            .iter()
            .filter(|shape| !new.iter().any(|new| kept(shape, new)))
            .chain(
                new.iter()
                    .filter(|shape| !old.iter().any(|old| kept(old, shape))),
            )
            .collect();
        let mut needed = crate::damage::Region::NOTHING;
        for shape in &changed {
            needed.add(crate::damage::bounds(shape).intersect(viewport));
        }
        if !everything && moved.is_none() && detecting_over_repaint() {
            let area = |region: &crate::damage::Region| {
                region
                    .rects()
                    .iter()
                    .map(|rect| rect.width() * rect.height())
                    .sum::<f32>()
            };
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
        for shape in changed {
            let bounds = crate::damage::bounds(shape).intersect(viewport);
            assert!(
                paint::uncovered(bounds, landed)
                    .into_iter()
                    .all(|rect| covered(rect, region)),
                "a shape changed outside the damaged region: {bounds:?} is not within {:?}",
                region.rects(),
            );
        }
        if let Some(moved) = moved {
            verify_move(previous, &shapes, region, moved);
        }
    }

    pub fn watch_size(&mut self, id: NodeId) -> ::reactive::ReadSignal<Vec2> {
        if let Some(watcher) = self.sizes.get(&id).and_then(|watchers| watchers.first()) {
            return watcher.read.clone();
        }
        let size = self.rects.get(&id).map_or(Vec2::ZERO, |rect| rect.size());
        let (read, write) = ::reactive::create_signal(size);
        self.sizes.get_or_default(id).push(SizeWatcher {
            read: read.clone(),
            write,
        });
        read
    }

    pub fn watch_placement(&mut self, id: NodeId) -> ::reactive::ReadSignal<Rect> {
        if let Some(watcher) = self
            .placements
            .get(&id)
            .and_then(|watchers| watchers.first())
        {
            return watcher.read.clone();
        }
        let rect = self.rects.get(&id).unwrap_or(Rect::ZERO);
        let (read, write) = ::reactive::create_signal(rect);
        self.placements.get_or_default(id).push(PlacementWatcher {
            read: read.clone(),
            write,
        });
        read
    }

    pub fn register_placement_watcher(
        &mut self,
        id: NodeId,
        read: ::reactive::ReadSignal<Rect>,
        write: ::reactive::WriteSignal<Rect>,
    ) {
        self.placements
            .get_or_default(id)
            .push(PlacementWatcher { read, write });
    }

    pub fn register_size_watcher(
        &mut self,
        id: NodeId,
        read: ::reactive::ReadSignal<Vec2>,
        write: ::reactive::WriteSignal<Vec2>,
    ) {
        self.sizes
            .get_or_default(id)
            .push(SizeWatcher { read, write });
    }

    pub fn name_component(&mut self, id: NodeId, name: &'static str) {
        self.component_names.entry(id).or_default().push(name);
    }

    pub fn component_names(&self, id: NodeId) -> &[&'static str] {
        self.component_names.get(&id).map_or(&[], Vec::as_slice)
    }

    pub fn set_component_state_dyn(&mut self, id: NodeId, state: Box<dyn Any>) {
        self.component_states.entry(id).or_default().push(state);
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

    pub fn deliver_constraint(&mut self, id: NodeId, available: Vec2) {
        if !self.delivering {
            return;
        }
        let Some(watchers) = self.sizes.get(&id) else {
            return;
        };
        self.constrained.insert(id);
        let writes: Vec<(::reactive::WriteSignal<Vec2>, Vec2)> = watchers
            .iter()
            .filter_map(|watcher| {
                let held = watcher.read.get_untracked();
                let offered = constrained(held, available);
                (offered != held).then(|| (watcher.write.clone(), offered))
            })
            .collect();
        if writes.is_empty() {
            return;
        }
        ::reactive::settle(|| {
            for (write, size) in writes {
                write.set(size);
            }
        });
    }

    pub fn deliver_unmeasured_constraint(&mut self, id: NodeId, available: Vec2) {
        if self.constrained.contains(&id) {
            return;
        }
        self.deliver_constraint(id, available);
    }

    pub fn deliver_placement(&mut self, id: NodeId, rect: Rect) {
        if !self.delivering {
            return;
        }
        let Some(watchers) = self.placements.get(&id) else {
            return;
        };
        let writes: Vec<::reactive::WriteSignal<Rect>> = watchers
            .iter()
            .filter(|watcher| watcher.read.get_untracked() != rect)
            .map(|watcher| watcher.write.clone())
            .collect();
        if writes.is_empty() {
            return;
        }
        ::reactive::settle(|| {
            for write in writes {
                write.set(rect);
            }
        });
    }

    pub fn settle_effects(&mut self) {
        if self.delivering {
            ::reactive::settle(|| {});
        }
    }

    pub fn assert_confined(&self, id: NodeId, watermark: usize) {
        if cfg!(debug_assertions) {
            for changed in self.arena.relaid_since(watermark) {
                assert!(
                    *changed == id || self.placed_pass.get(changed) != Some(&self.layout_pass),
                    "laying out {id:?} ({}) restructured {changed:?} ({}), which this pass had already placed",
                    self.node_kind(id),
                    self.node_kind(*changed),
                );
            }
        }
    }

    pub fn measured(&mut self, id: NodeId, available: Vec2) -> Option<Vec2> {
        if self.arena.stale(id) {
            self.measurements.remove(&id);
            self.baselines.remove(&id);
            return None;
        }
        let held = self.measurements.get(&id)?;
        held.iter()
            .find(|(offered, _)| same_size(*offered, available))
            .or_else(|| {
                held.iter()
                    .find(|(offered, size)| still_fits(*offered, *size, available))
            })
            .map(|(_, size)| *size)
    }

    pub fn remember_measurement(
        &mut self,
        id: NodeId,
        available: Vec2,
        size: Vec2,
        watermark: u64,
    ) {
        if self.arena.layout_revision != watermark {
            return;
        }
        self.arena.clear_stale(id);
        let held = self.measurements.get_or_default(id);
        held.retain(|(offered, _)| !same_size(*offered, available));
        if held.len() >= REMEMBERED_MEASUREMENTS {
            held.remove(0);
        }
        held.push((available, size));
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

    pub fn note_parent(&mut self, id: NodeId) {
        if !self.delivering {
            return;
        }
        let parent = self.layout_parent;
        self.arena.set_parent(id, parent);
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
        if self.delivering {
            let mut dropped = Vec::new();
            for child in self.dropped_children(id, frame.base) {
                self.drop_placement(child, out, &mut dropped);
            }
            for node in dropped {
                self.release_placement(node);
            }
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
        if self.delivering && self.reached_pass.get(&id) == Some(&self.layout_pass) {
            return;
        }
        self.painters.remove(&id);
        let bounds = self.paint_cache.borrow().absolute_bounds(id);
        if out.remove(&id).is_some() {
            self.accessibility_tree.get_mut().mark(id, &self.arena);
            self.damage.add(bounds);
        }
        dropped.push(id);
        if self.delivering {
            self.deliver_placed(id, false);
        }
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

    pub fn take_interact_pool(&mut self) -> Vec<Vec<NodeId>> {
        std::mem::take(&mut self.interact_pool)
    }

    pub fn put_back_interact_pool(&mut self, pool: Vec<Vec<NodeId>>) {
        self.interact_pool = pool;
    }

    pub fn laying_out(&self) -> Option<NodeId> {
        self.layout_parent
    }

    pub fn delivering(&self) -> bool {
        self.delivering
    }

    pub fn invalidate_measurement(&mut self, id: NodeId) {
        self.arena.invalidate_node(id);
    }

    pub fn enter_scroll_host(&mut self, id: NodeId) {
        self.scroll_hosts.push(id);
    }

    pub fn leave_scroll_host(&mut self) {
        self.scroll_hosts.pop();
    }

    pub fn record_scroll_shift(&mut self, shift: f32) {
        let Some(&host) = self.scroll_hosts.last() else {
            return;
        };
        *self.scroll_shifts.get_or_default(host) += shift;
    }

    pub fn take_scroll_shift(&mut self, id: NodeId) -> f32 {
        self.scroll_shifts.remove(&id).unwrap_or(0.0)
    }

    pub fn placing_len(&self) -> usize {
        self.placing.len()
    }

    pub fn rewind_placing(&mut self, len: usize) {
        if self.delivering {
            self.placing.truncate(len);
        }
    }

    pub fn note_placed(&mut self, id: NodeId) {
        if self.delivering {
            self.placing.push(id);
            self.reached_pass.insert(id, self.layout_pass);
            self.deliver_placed(id, true);
        }
    }

    pub fn watch_placed(&mut self, id: NodeId) -> ::reactive::ReadSignal<bool> {
        if let Some((read, _)) = self.placed.get(&id) {
            return read.clone();
        }
        let (read, write) = ::reactive::create_signal(self.rects.contains_key(&id));
        self.placed.insert(id, (read.clone(), write));
        read
    }

    fn deliver_placed(&mut self, id: NodeId, placed: bool) {
        let Some((read, write)) = self.placed.get(&id) else {
            return;
        };
        if read.get_untracked() == placed {
            return;
        }
        let write = write.clone();
        ::reactive::settle(|| write.set(placed));
    }

    pub fn reuse_placement(
        &mut self,
        id: NodeId,
        rect: Rect,
        given: PainterState,
        own: PainterState,
        out: &Rects,
    ) -> bool {
        let reusable = self.delivering
            && !self.arena.unplaced(id)
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
        let space = out.set_space(SpaceId::of(id), given.space, rect.min.to_vec2(), given.clip);
        if moved || space {
            self.spaces_moved = true;
            self.accessibility_tree.get_mut().mark(id, &self.arena);
        }
        true
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
        if !self.delivering {
            return;
        }
        self.arena.clear_unplaced(id);
        self.arena.note_relaid(id);
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
        if previous != Some(placed) || previous_held != Some((given.clip, culled)) {
            self.accessibility_tree.get_mut().mark(id, &self.arena);
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

    pub fn culls(&self, id: NodeId, size: Vec2, own: PainterState) -> bool {
        self.delivering && !Sight::shows(size, own) && !self.holds_focus(id)
    }

    fn holds_focus(&self, id: NodeId) -> bool {
        let Some(focused) = self.focused else {
            return false;
        };
        self.descends(focused, id) || self.descends(id, focused)
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

    fn descends(&self, node: NodeId, from: NodeId) -> bool {
        let mut current = Some(node);
        for _ in 0..ANCESTOR_LIMIT {
            match current {
                Some(node) if node == from => return true,
                Some(node) => current = self.arena.parent(node),
                None => return false,
            }
        }
        false
    }

    pub fn note_space_reads(&mut self, id: NodeId, reads: bool, base: usize) {
        if !self.delivering {
            return;
        }
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
        let space = SpaceId::inside(host, slot);
        let entered = painter.shifted(Some(space), translation, clip);
        let state = painter.state();
        let kept = state.clip.intersect(clip);
        if out.set_space(space, state.space, translation, kept) && self.delivering {
            self.spaces_moved = true;
            self.accessibility_tree.get_mut().mark(host, &self.arena);
        }
        entered
    }

    fn redeliver_placements(&mut self) {
        let moved: Vec<(::reactive::WriteSignal<Rect>, Rect)> = self
            .placements
            .iter()
            .filter_map(|(id, watchers)| Some((self.rects.get(&id)?, watchers)))
            .flat_map(|(rect, watchers)| {
                watchers
                    .iter()
                    .filter(move |watcher| watcher.read.get_untracked() != rect)
                    .map(move |watcher| (watcher.write.clone(), rect))
            })
            .collect();
        if moved.is_empty() {
            return;
        }
        ::reactive::settle(|| {
            for (write, rect) in moved {
                write.set(rect);
            }
        });
    }

    pub fn viewport_rect(&self) -> Rect {
        self.viewport
            .as_ref()
            .map_or(Rect::NOTHING, |(_, rect, _)| *rect)
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.viewport.as_ref().map_or(1.0, |(_, _, scale)| *scale)
    }

    pub fn layout_text(&self, text: &str, font: FontId, layout: TextLayout) -> Option<Galley> {
        let (ctx, _, _) = self.viewport.as_ref()?;
        Some(ctx.painter().layout_text(text, font, layout))
    }

    pub fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::new(self.pixels_per_point())
    }

    pub fn measure_root(&mut self, ctx: &Context, available: Vec2) -> Option<Vec2> {
        let root = self.root?;
        let painter = ctx.painter();
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

    fn update_layout(&mut self, ctx: &Context, rect: Rect) -> bool {
        if self.layout_revision == self.arena.layout_revision {
            return false;
        }
        for _ in 0..LAYOUT_PASSES {
            self.lay_out_pass(ctx, rect);
            let overlays_moved = self.relay_moved_overlays();
            let unsettled = self.root.is_some_and(|root| self.arena.unplaced(root));
            if !std::mem::take(&mut self.spaces_moved) {
                if unsettled || overlays_moved {
                    continue;
                }
                break;
            }
            let context = self.reactive_scope().context();
            {
                let _guard = crate::current::install(self);
                context.run(|| {
                    crate::current::with_document(Document::redeliver_placements);
                });
            }
            if !unsettled && self.layout_revision == self.arena.layout_revision {
                break;
            }
        }
        for (fade, edges) in std::mem::take(&mut self.deferred_fades) {
            if self.arena.contains(fade.id()) {
                self.set_fade(fade, edges);
            }
        }
        for node in std::mem::take(&mut self.deferred_reveals) {
            if self.arena.contains(node) {
                self.reveal_node(node);
            }
        }
        true
    }

    fn lay_out_pass(&mut self, ctx: &Context, rect: Rect) {
        self.layout_pass = self.layout_pass.wrapping_add(1);
        let rects = Rc::clone(&self.rects);
        self.placing.clear();
        if let Some(root) = self.root {
            let painter = ctx.painter();
            let context = self.reactive_scope().context();
            let placed = &*rects;
            self.constrained.clear();
            self.layout_parent = None;
            self.delivering = true;
            {
                let _guard = crate::current::install(self);
                context.run(|| {
                    crate::current::with_document(|document| {
                        layout::layout(document, &painter, root, rect, placed);
                    });
                });
            }
            self.delivering = false;
        }
        self.placing.clear();
        self.lay_out_boundaries(ctx, &rects);
        self.layout_revision = self.arena.layout_revision;
    }
}

const LAYOUT_PASSES: usize = 3;
const LAID_OUT_ROUNDS: usize = 4;

fn covered(rect: Rect, region: &Region) -> bool {
    let mut left = vec![rect];
    for damaged in region.rects() {
        left = left
            .into_iter()
            .flat_map(|rect| paint::uncovered(rect, damaged.expand(0.01)))
            .collect();
    }
    left.iter().all(|rect| !rect.is_positive())
}

fn verify_move(previous: &[Shape], shapes: &[Shape], region: &Region, moved: Moved) {
    let landed = moved.from.translate(moved.by);
    let within = |shape: &Shape, area: Rect| crate::damage::bounds(shape).intersects(area);
    let shown = |shapes: &[Shape], area: Rect| {
        shapes
            .iter()
            .rposition(|shape| {
                paint::covers(shape, area)
                    && matches!(shape, Shape::Rect { color, .. } if color.alpha() == u8::MAX)
            })
            .unwrap_or(0)
    };
    let (previous, shapes) = (
        &previous[shown(previous, moved.from)..],
        &shapes[shown(shapes, landed)..],
    );
    let before: Vec<Shape> = previous
        .iter()
        .filter(|shape| within(shape, moved.from))
        .map(|shape| crate::painter::placed_shape(shape, moved.by, Rect::EVERYTHING))
        .collect();
    let after: Vec<&Shape> = shapes
        .iter()
        .filter(|shape| within(shape, landed))
        .collect();
    let matches = |old: &Shape, new: &Shape| {
        (paint::covers(old, landed) && paint::covers(new, landed) && same_fill(old, new))
            || (unclipped(old) == unclipped(new)
                && clip_of(old).intersect(landed) == clip_of(new).intersect(landed))
    };
    let damaged = |shape: &Shape| {
        paint::painted(shape, crate::damage::bounds(shape))
            .into_iter()
            .all(|rect| covered(rect.intersect(landed), region))
    };
    for new in &after {
        assert!(
            damaged(new) || before.iter().any(|old| matches(old, new)),
            "a shape the copy moved into {:?} was not there to be moved",
            crate::damage::bounds(new).intersect(landed),
        );
    }
    for old in &before {
        assert!(
            damaged(old) || after.iter().any(|new| matches(old, new)),
            "the copy moved a shape into {:?} that is no longer painted there",
            crate::damage::bounds(old).intersect(landed),
        );
    }
}

fn unclipped(shape: &Shape) -> Shape {
    let mut shape = shape.clone();
    *crate::context::shape_clip(&mut shape) = Rect::EVERYTHING;
    shape
}

fn clip_of(shape: &Shape) -> Rect {
    *crate::context::shape_clip(&mut shape.clone())
}

fn same_fill(left: &Shape, right: &Shape) -> bool {
    matches!(
        (left, right),
        (Shape::Rect { color, .. }, Shape::Rect { color: other, .. }) if color == other
    )
}

fn flatten(painting: &[(Rc<Display>, Entry)]) -> Vec<Shape> {
    let mut main = Vec::new();
    let mut top = Vec::new();
    for (display, entry) in painting {
        display.flatten(*entry, &mut main, &mut top);
        main.append(&mut top);
    }
    main
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
            let state = PainterState {
                origin: rects.offset(held.given.space),
                space_clip: rects.space_clip(held.given.space),
                list: Entry::NONE,
                ..held.given
            };
            let painter = Painter::resumed(ctx.clone(), state);
            let rect = placed.rect;
            let context = self.reactive_scope().context();
            self.layout_parent = self.arena.parent(boundary);
            self.delivering = true;
            {
                let _guard = crate::current::install(self);
                context.run(|| {
                    crate::current::with_document(|document| {
                        layout::layout(document, &painter, boundary, rect, rects);
                    });
                });
            }
            self.delivering = false;
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
