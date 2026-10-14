use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::time::Instant;

use accesskit::Node;

use crate::accessibility::{self, AccessibilityTree};
use crate::base::child_list::{ChildHost, NodeChildren, SlotId};
use crate::context::{Context, Moved};
use crate::damage::{Damage, Region};
use crate::file_picker::{FileFilter, FilePick, FilePickId};
use crate::flash::FlashLog;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::input::{Key, KeyChord, Modifiers};

use crate::display::Display;
use crate::interact;
use crate::layout;
use crate::node::{
    ANCESTOR_LIMIT, Arena, Element, NodeId, NodeMap, NodeOf, Placed, Rects, SpaceId,
};
use crate::paint::{self, PaintCache};
use crate::painter::{Local, Painter, PainterState, Shape};
use crate::performance::{FrameMeasurement, FrameWork, PerformanceSnapshot, PerformanceTracker};
use crate::pixel_grid::PixelGrid;
use crate::screen_simulation::{self, Placement};
use crate::screens::Screen;
use crate::sight::Sight;

type PickedCallback = Box<dyn FnOnce(FilePick)>;
pub type FingerTap = dyn Fn(usize) -> bool;

pub type ActionsHook = dyn Fn(&Context, &[NodeId]);

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
    pub(crate) focus_before: Option<NodeId>,
    pub composed: String,
    pub(crate) keyboard_held: bool,
    pub(crate) keyboard_asks: u32,
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
    pub(crate) layer_state: crate::layer::OpenLayers,
    pub(crate) focus_scopes: crate::base::focus_scope::FocusScopes,
    pub(crate) focus_dirty: bool,
    pub(crate) focus_wanted: Option<NodeId>,
    pub back_handlers: Vec<NodeOf<crate::base::back::BackNode>>,
    pub(crate) back_gesture: Option<crate::layer::DismissTarget>,
    timers: RefCell<crate::timer::Timers>,
    scale: ::reactive::OwnedSignal<f32>,
    attached: ::reactive::OwnedSignal<u64>,
    focus_visible: ::reactive::OwnedSignal<bool>,
    fullscreen: ::reactive::OwnedSignal<bool>,
    screens: ::reactive::OwnedSignal<Vec<Screen>>,
    offered_screens: Option<Vec<Screen>>,
    rescreened: RefCell<Option<Vec<Screen>>>,
    reattached: Cell<bool>,
    finger_taps: RefCell<Vec<Weak<FingerTap>>>,
    pub keys: crate::keys::KeyListeners,
    actions_hooks: RefCell<Vec<Weak<ActionsHook>>>,
    pub(crate) tapping: Option<Key>,
    pub(crate) keyboard: bool,
    input_frames: u64,
    modifiers: ::reactive::OwnedSignal<Modifiers>,
    intercepted_keys: ::reactive::OwnedSignal<Vec<KeyChord>>,
    pub autoscroll: Option<crate::interact::autoscroll::Autoscroll>,
    pub(crate) press_claimants: HashSet<NodeId>,
    pub(crate) outside_watchers: HashSet<NodeId>,
    pub forward: crate::interact::forward::Routing,
    pub pointers: crate::interact::pointer::Pointers,
    pub drags: Rc<crate::drag_board::Board>,
    paste_requested: bool,
    unsent_file_picks: Vec<(FileFilter, PickedCallback)>,
    waiting_file_picks: Vec<(FilePickId, PickedCallback)>,
    test_ids: HashMap<String, Vec<NodeId>>,
    node_test_ids: HashMap<NodeId, Vec<String>>,
    layout_revision: u64,
    paint_revision: u64,
    reports: Vec<Box<dyn FnOnce()>>,
    invalidated: Vec<NodeId>,
    interaction_done: Option<Rc<dyn Fn()>>,
    laid_out: Option<Rc<dyn Fn()>>,
    measurements: NodeMap<Vec<(Vec2, Vec2)>>,
    baselines: NodeMap<Vec<(Vec2, Option<f32>)>>,
    layout_parent: Option<NodeId>,
    placed_children: NodeMap<Vec<NodeId>>,
    placing: Vec<NodeId>,
    pub interact_bounds: NodeMap<Rect>,
    pub interact_bounds_version: Option<u64>,
    pub placed_pass: NodeMap<u64>,
    pub reached_pass: NodeMap<u64>,
    layout_pass: u64,
    scroll_hosts: Vec<NodeId>,
    scroll_shifts: NodeMap<f32>,
    pub(crate) reveals: NodeMap<NodeId>,
    viewport: Option<(Context, Rect, f32)>,
    fonts_generation: u64,
    painting: Vec<(Rc<Display>, Vec2)>,
    pub paint_cache: RefCell<PaintCache>,
    pub verifies_paint: bool,
    pub copied_text: Option<String>,
    next_paint: Option<Instant>,
    now: Instant,
    reactive_scope: ::reactive::Scope,
    zone: u64,
    extensions: HashMap<std::any::TypeId, Box<dyn Any>>,
    node_refs: HashMap<NodeId, Vec<Weak<Cell<Option<NodeId>>>>>,
    placements: NodeMap<::reactive::OwnedSignal<Rect>>,
    placed: NodeMap<::reactive::OwnedSignal<bool>>,
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
    rubber_banding: bool,
}

#[derive(Clone, Copy)]
struct Placing {
    given: PainterState,
    own: PainterState,
    sight: Sight,
    culled: bool,
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

static NEXT_ZONE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Document {
    pub fn new() -> Self {
        let rects = Rc::new(Rects::default());
        let zone = NEXT_ZONE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            arena: Arena::default(),
            root: None,
            focused: None,
            focus_before: None,
            composed: String::new(),
            keyboard_held: false,
            keyboard_asks: 0,
            activated: None,
            activation_key: None,
            drags: Rc::new(crate::drag_board::Board::new(Rc::clone(&rects))),
            rects,
            tools: None,
            inspector_requested: false,
            screen_pointer: None,
            last_pointer: None,
            acted: Acted::Nothing,
            placement: None,
            portal_holders: std::collections::HashMap::new(),
            layer_state: Default::default(),
            focus_scopes: Default::default(),
            focus_dirty: false,
            focus_wanted: None,
            back_handlers: Vec::new(),
            back_gesture: None,
            timers: RefCell::new(Vec::new()),
            scale: ::reactive::OwnedSignal::new(1.0),
            attached: ::reactive::OwnedSignal::new(0),
            focus_visible: ::reactive::OwnedSignal::new(false),
            fullscreen: ::reactive::OwnedSignal::new(false),
            screens: ::reactive::OwnedSignal::new(Vec::new()),
            offered_screens: None,
            rescreened: RefCell::new(None),
            reattached: Cell::new(false),
            finger_taps: RefCell::new(Vec::new()),
            keys: Default::default(),
            actions_hooks: RefCell::new(Vec::new()),
            tapping: None,
            keyboard: true,
            input_frames: 0,
            modifiers: ::reactive::OwnedSignal::new(Modifiers::NONE),
            intercepted_keys: ::reactive::OwnedSignal::new(Vec::new()),
            autoscroll: None,
            press_claimants: HashSet::new(),
            outside_watchers: HashSet::new(),
            forward: Default::default(),
            pointers: Default::default(),
            paste_requested: false,
            unsent_file_picks: Vec::new(),
            waiting_file_picks: Vec::new(),
            test_ids: HashMap::new(),
            node_test_ids: HashMap::new(),
            layout_revision: 0,
            paint_revision: 0,
            reports: Vec::new(),
            invalidated: Vec::new(),
            interaction_done: None,
            laid_out: None,
            measurements: NodeMap::default(),
            baselines: NodeMap::default(),
            layout_parent: None,
            placed_children: NodeMap::default(),
            placing: Vec::new(),
            interact_bounds: NodeMap::default(),
            interact_bounds_version: None,
            placed_pass: NodeMap::default(),
            reached_pass: NodeMap::default(),
            layout_pass: 0,
            scroll_hosts: Vec::new(),
            scroll_shifts: NodeMap::default(),
            reveals: NodeMap::default(),
            viewport: None,
            fonts_generation: 0,
            painting: Vec::new(),
            paint_cache: RefCell::new(PaintCache::default()),
            verifies_paint: true,
            copied_text: None,
            next_paint: None,
            now: Instant::now(),
            reactive_scope: ::reactive::Scope::new(),
            zone,
            extensions: HashMap::new(),
            node_refs: HashMap::new(),
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
        self.attached.read()
    }

    pub fn watch_pixels_per_point(&self) -> ::reactive::ReadSignal<f32> {
        self.scale.read()
    }

    pub fn watch_fullscreen(&self) -> ::reactive::ReadSignal<bool> {
        self.fullscreen.read()
    }

    pub fn watch_screens(&self) -> ::reactive::ReadSignal<Vec<Screen>> {
        self.screens.read()
    }

    pub fn screens(&self) -> Vec<Screen> {
        self.screens.read().get_untracked()
    }

    pub fn screen_at(&self, pos: Pos2) -> Option<Screen> {
        self.screens
            .read()
            .with_untracked(|screens| crate::screens::at(screens, pos).cloned())
    }

    pub fn screen_under(&self, rect: Rect) -> Option<Screen> {
        self.screens
            .read()
            .with_untracked(|screens| crate::screens::under(screens, rect).cloned())
    }

    pub fn screen_named(&self, id: &str) -> Option<Screen> {
        self.screens
            .read()
            .with_untracked(|screens| screens.iter().find(|screen| screen.id == id).cloned())
    }

    pub fn watch_focus_visible(&self) -> ::reactive::ReadSignal<bool> {
        self.focus_visible.read()
    }

    pub fn set_focus_visible(&self, visible: bool) {
        self.focus_visible.write().set(visible);
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
        for hook in live {
            hook(ctx, &path);
        }
    }

    pub fn listen_keys(
        &self,
        at: crate::keys::KeyAt,
        phase: crate::keys::KeyPhase,
        reveal: bool,
        handler: Weak<crate::keys::KeyHandler>,
    ) {
        self.keys.listen(at, phase, reveal, handler);
    }

    pub fn input_frames(&self) -> u64 {
        self.input_frames
    }

    pub(crate) fn note_input(&mut self) {
        self.input_frames += 1;
    }

    pub fn watch_modifiers(&self) -> ::reactive::ReadSignal<Modifiers> {
        self.modifiers.read()
    }

    pub fn intercepted_keys_writer(&self) -> ::reactive::WriteSignal<Vec<KeyChord>> {
        self.intercepted_keys.write()
    }

    pub fn intercepted_keys(&self) -> Vec<KeyChord> {
        self.intercepted_keys.read().get_untracked()
    }

    pub fn set_modifiers(&self, modifiers: Modifiers) {
        if self.modifiers.read().get_untracked() != modifiers {
            self.modifiers.write().set(modifiers);
        }
    }

    fn run_timers(&self) {
        let scale = self.pixels_per_point();
        if self.scale.read().get_untracked() != scale {
            self.scale.write().set(scale);
        }
        let fullscreen = self
            .viewport
            .as_ref()
            .is_some_and(|(ctx, _, _)| ctx.fullscreen());
        if self.fullscreen.read().get_untracked() != fullscreen {
            self.fullscreen.write().set(fullscreen);
        }
        let rescreened = self.rescreened.borrow_mut().take();
        if let Some(screens) = rescreened {
            self.screens.write().set(screens);
        }
        if self.reattached.replace(false) {
            self.attached.write().update(|attached| *attached += 1);
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
            self.arena.touch(id);
        }
    }

    pub fn copy_text(&mut self, text: impl Into<String>) {
        self.copied_text = Some(text.into());
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

    pub(crate) fn drain_removals(&mut self) {
        while self.arena.removals_pending() {
            let context = self.reactive_scope().context();
            let removals = self.arena.take_removals();
            let guard = crate::current::install(self);
            let installed = matches!(guard, crate::current::DocumentGuard::Installed(..));
            match installed {
                true => context.run(|| {
                    crate::current::with_document(|document| document.remove_nodes(&removals))
                }),
                false => {
                    drop(guard);
                    self.remove_nodes(&removals);
                }
            }
        }
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.remove_nodes(&[id]);
    }

    fn remove_nodes(&mut self, ids: &[NodeId]) {
        let leaving: HashSet<NodeId> = ids.iter().copied().collect();
        let mut parents: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
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
            self.focused = None;
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
        self.back_handlers.retain(|handler| handler.id() != id);
        self.paint_cache.borrow_mut().forget(id);
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
            self.focused = None;
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
        self.show_screen(ctx, content);
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
        let rescreened = self
            .screens
            .read()
            .with_untracked(|shown| *shown != screens);
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
        self.deliver_file_picks(ctx);

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
        if self.layers().takes_keyboard() {
            ctx.want_keyboard();
        }
        self.intercepted_keys
            .read()
            .with_untracked(|chords| ctx.intercept_keys(chords));

        if let Some(text) = self.copied_text.take() {
            ctx.copy_text(text);
        }
        if std::mem::take(&mut self.paste_requested) {
            ctx.request_paste();
        }
        ctx.accept_pastes(&self.paste_kinds());
        if resized && self.focus_takes_text() {
            self.reveal_focus();
        }
        measurement.layout_passes =
            FrameMeasurement::measure(&mut measurement.timings.layout, || {
                self.update_layout(ctx, rect)
            });
        if self.keyboard
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
            let cache = self.paint_cache.get_mut();
            let moves = cache.take_moves();
            let stale = cache.take_stale();
            let everything = self.damage.is_everything();
            let region = self.damage.take(rect);
            let (region, moved) = match everything {
                true => (region, None),
                false => self.settle_moves(moves, stale, region, rect),
            };
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
        for overlay in self.layers().iter().filter_map(|layer| layer.overlay) {
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
        stale: Vec<paint::Stale>,
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
        let only = &moves[largest];
        let visible = only.viewport.intersect(viewport);
        let (fixed, inner) = paint::fixed_damage(&self.painting, only.space, visible, only.by);
        let shown = visible.intersect(inner);
        let landed = shown.intersect(shown.translate(only.by));
        let source = landed.translate(-only.by);
        let cache = self.paint_cache.borrow();
        let mut damaged: Vec<Rect> = region.rects().to_vec();
        for stale in &stale {
            let moved = stale.rect.intersect(source).translate(only.by);
            match (
                cache.scrolls_within(stale.scroll, only.space),
                stale.settled,
            ) {
                (true, true) => damaged.push(stale.rect),
                (true, false) => {
                    damaged.extend(paint::uncovered(stale.rect, source));
                    damaged.push(moved);
                }
                (false, _) => damaged.extend([stale.rect, moved]),
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
        let fresh = std::mem::replace(&mut self.painting, painting);
        self.paint_cache.replace(retained);
        self.damage = damage;
        self.work.painted_nodes.set(counted.0);
        self.work.replayed_nodes.set(counted.1);
        let shapes = flatten(&self.painting);
        assert!(
            fresh.len() == self.painting.len()
                && fresh
                    .iter()
                    .zip(&self.painting)
                    .all(|((fresh, at), (retained, held))| at == held && fresh.same_tree(retained)),
            "the retained painting differs from painting from scratch: {:?} against {:?}",
            flatten(&fresh)
                .iter()
                .zip(&shapes)
                .position(|(fresh, retained)| !fresh.same_look(retained))
                .map(|at| crate::damage::bounds(&shapes[at])),
            fresh
                .iter()
                .map(|(display, at)| display.bounds.translate(*at))
                .collect::<Vec<_>>(),
        );
        let kept = |old: &Shape, new: &Shape| {
            old.same_look(new) || paint::redrawn_in_place(old, new).is_some()
        };
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

    pub fn watch_placement(&mut self, id: NodeId) -> ::reactive::ReadSignal<Rect> {
        if let Some(watched) = self.placements.get(&id) {
            return watched.read();
        }
        let watched = ::reactive::OwnedSignal::new(self.rects.get(&id).unwrap_or(Rect::ZERO));
        let read = watched.read();
        self.placements.insert(id, watched);
        read
    }

    pub(crate) fn watch_placement_with(
        &mut self,
        id: NodeId,
        watched: ::reactive::OwnedSignal<Rect>,
    ) {
        self.placements.insert(id, watched);
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
            self.paint_cache.get_mut().unplace(id);
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
        self.placing.truncate(len);
    }

    pub fn note_placed(&mut self, id: NodeId) {
        self.placing.push(id);
        self.reached_pass.insert(id, self.layout_pass);
    }

    pub fn watch_placed(&mut self, id: NodeId) -> ::reactive::ReadSignal<bool> {
        if let Some(watched) = self.placed.get(&id) {
            return watched.read();
        }
        let watched = ::reactive::OwnedSignal::new(self.rects.contains_key(&id));
        let read = watched.read();
        self.placed.insert(id, watched);
        read
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
        let space = out.set_space(SpaceId::of(id), given.space, rect.min.to_vec2(), given.clip);
        if moved || space {
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
        !Sight::shows(size, own) && !self.holds_focus(id)
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
        let entered = painter.shifted(space, translation, clip);
        let state = painter.state();
        let kept = state.clip.intersect(clip);
        if out.set_space(space, state.space, translation, kept) {
            self.accessibility_tree.get_mut().mark(host, &self.arena);
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
        let rects = Rc::clone(&self.rects);
        let moved: Vec<(::reactive::WriteSignal<Rect>, Rect)> = self
            .placements
            .iter()
            .filter_map(|(id, watched)| Some((rects.get(&id)?, watched)))
            .filter(|(rect, watched)| watched.read().get_untracked() != *rect)
            .map(|(rect, watched)| (watched.write(), rect))
            .collect();
        let placed: Vec<(::reactive::WriteSignal<bool>, bool)> = self
            .placed
            .iter()
            .map(|(id, watched)| (rects.contains_key(&id), watched))
            .filter(|(placed, watched)| watched.read().get_untracked() != *placed)
            .map(|(placed, watched)| (watched.write(), placed))
            .collect();
        if reports.is_empty() && moved.is_empty() && placed.is_empty() {
            return;
        }
        crate::current::enter(self, || {
            for report in reports {
                report();
            }
            for (write, rect) in moved {
                write.set(rect);
            }
            for (write, shown) in placed {
                write.set(shown);
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
        eprintln!(
            "beui: layout did not settle in {LAYOUTS_PER_FRAME} layouts; what each reported kept \
             changing the layout of {relaid:?}"
        );
    }

    fn run_laid_out(&mut self) {
        if let Some(laid_out) = self.laid_out.clone() {
            let context = self.reactive_scope().context();
            let _guard = crate::current::install(self);
            context.run(|| laid_out());
        }
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

    fn needs_layout(&self) -> bool {
        self.layout_revision != self.arena.layout_revision
            || self.root.is_some_and(|root| self.arena.unplaced(root))
    }

    fn update_layout(&mut self, ctx: &Context, rect: Rect) -> usize {
        let mut passes = 0;
        if self.needs_layout() {
            self.lay_out_tree(ctx, rect);
            passes += 1;
        }
        let mut watermark = self.arena.relaid_len();
        self.report_laid_out();
        self.run_laid_out();
        while self.needs_layout() {
            if passes == LAYOUTS_PER_FRAME {
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
            let painter = ctx.painter();
            let placed = &*rects;
            self.layout_parent = None;
            crate::current::run_in(self, |document| {
                layout::layout(document, &painter, root, rect, placed);
                document.lay_out_boundaries(ctx, placed);
            });
        }
        self.placing.clear();
        self.layout_revision = self.arena.layout_revision;
        self.layout_pass = self.layout_pass.wrapping_add(1);
    }
}

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

fn flatten(painting: &[(Rc<Display>, Vec2)]) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for (display, at) in painting {
        display.flatten(*at, &mut shapes);
    }
    shapes
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

const LAYOUTS_PER_FRAME: usize = 4;

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
                local: Local::NONE,
                ..held.given
            };
            let painter = Painter::resumed(ctx.clone(), state);
            self.layout_parent = self.arena.parent(boundary);
            layout::layout(self, &painter, boundary, placed.rect, rects);
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
