use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::time::Instant;

use accesskit::Node;

use crate::accessibility::{self, AccessibilityTree};
use crate::base::child_list::{ChildHost, SlotId};
use crate::context::Context;
use crate::damage::Damage;
use crate::flash::FlashLog;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::input::{BackEdge, Event, Key, KeyPress};

use crate::inspector::{Inspector, Layout};
use crate::interact::{self, Keys};
use crate::layout;
use crate::node::{Arena, NodeId, NodeMap, Rects};
use crate::paint::{self, PaintCache};
use crate::painter::{Painter, PainterState, Shape};
use crate::performance::{FrameMeasurement, FrameWork, PerformanceSnapshot, PerformanceTracker};
use crate::pixel_grid::PixelGrid;
use crate::screen_simulation::{self, Placement};
use crate::styled::{Theme, ThemeStore};

pub(crate) type Shortcut = dyn Fn(KeyPress) -> bool;
pub(crate) type FingerTap = dyn Fn(usize) -> bool;

pub struct Document {
    pub(crate) arena: Arena,
    pub(crate) root: Option<NodeId>,
    pub(crate) focused: Option<NodeId>,
    pub(crate) activated: Option<NodeId>,
    pub(crate) activation_key: Option<Key>,
    pub(crate) rects: Rc<Rects>,
    pub(crate) inspector: Option<Box<Inspector>>,
    pub(crate) inspectable: bool,
    inspector_requested: bool,
    screen_pointer: Option<Pos2>,
    placement: Option<(Rect, Option<Placement>)>,
    pub(crate) portal_holders: std::collections::HashMap<NodeId, NodeId>,
    pub(crate) overlay_stack: Vec<NodeId>,
    pub(crate) passive_overlays: Vec<NodeId>,
    pub(crate) back_handlers: Vec<NodeId>,
    pub(crate) back_gesture: Option<(NodeId, BackEdge)>,
    timers: RefCell<crate::timer::Timers>,
    scale: (::reactive::ReadSignal<f32>, ::reactive::WriteSignal<f32>),
    attached: (::reactive::ReadSignal<u64>, ::reactive::WriteSignal<u64>),
    focus_visible: (::reactive::ReadSignal<bool>, ::reactive::WriteSignal<bool>),
    reattached: Cell<bool>,
    shortcuts: RefCell<Vec<Weak<Shortcut>>>,
    finger_taps: RefCell<Vec<Weak<FingerTap>>>,
    pub(crate) touch_scroll_vertical: Option<NodeId>,
    pub(crate) touch_shift: crate::geometry::Vec2,
    pub(crate) touch_scroll_horizontal: Option<NodeId>,
    pub(crate) wheel_latch: Option<(NodeId, Instant)>,
    pub(crate) autoscroll: Option<crate::interact::autoscroll::Autoscroll>,
    pub(crate) pointer_capture: Option<NodeId>,
    pub(crate) drags: Rc<crate::unstyled::DragBoard>,
    paste_requested: bool,
    test_ids: HashMap<String, Vec<NodeId>>,
    node_test_ids: HashMap<NodeId, Vec<String>>,
    layout_revision: u64,
    paint_revision: u64,
    delivering: bool,
    pub(crate) deferred_reveals: Vec<NodeId>,
    constrained: HashSet<NodeId>,
    measurements: NodeMap<Vec<(Vec2, Vec2)>>,
    layout_parent: Option<NodeId>,
    placed_children: NodeMap<Vec<NodeId>>,
    placing: Vec<NodeId>,
    interact_pool: Vec<Vec<NodeId>>,
    pub(crate) engaged: Vec<NodeId>,
    pub(crate) interact_parents: NodeMap<NodeId>,
    pub(crate) interact_bounds: NodeMap<Rect>,
    pub(crate) interact_bounds_version: Option<u64>,
    placed_pass: NodeMap<u64>,
    reached_pass: NodeMap<u64>,
    layout_pass: u64,
    scroll_hosts: Vec<NodeId>,
    scroll_shifts: NodeMap<f32>,
    viewport: Option<(Context, Rect, f32)>,
    shapes: Vec<Shape>,
    pub(crate) paint_cache: RefCell<PaintCache>,
    pub(crate) verifies_paint: bool,
    pub(crate) copied_text: Option<String>,
    next_paint: Option<Instant>,
    reactive_scope: ::reactive::Scope,
    theme: ThemeStore,
    node_scopes: HashMap<NodeId, Vec<::reactive::Scope>>,
    sizes: NodeMap<Vec<SizeWatcher>>,
    placements: NodeMap<Vec<PlacementWatcher>>,
    placed: NodeMap<(::reactive::ReadSignal<bool>, ::reactive::WriteSignal<bool>)>,
    component_states: HashMap<NodeId, Vec<Box<dyn Any>>>,
    component_names: HashMap<NodeId, Vec<&'static str>>,
    pub(crate) accessibility_id: u32,
    pub(crate) accessibility: NodeMap<Node>,
    pub(crate) accessibility_tree: RefCell<AccessibilityTree>,
    performance: PerformanceTracker,
    pub(crate) work: WorkCounters,
    changes: FlashLog<NodeId>,
    damage: Damage,
    damage_flashes: FlashLog<Rect>,
    painters: NodeMap<PainterState>,
    rubber_banding: bool,
}

struct SizeWatcher {
    read: ::reactive::ReadSignal<Vec2>,
    write: ::reactive::WriteSignal<Vec2>,
}

const REMEMBERED_MEASUREMENTS: usize = 4;

fn same_size(left: Vec2, right: Vec2) -> bool {
    left.x.to_bits() == right.x.to_bits() && left.y.to_bits() == right.y.to_bits()
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
pub(crate) struct WorkCounters {
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

    pub(crate) fn note_described(&self, nodes: usize) {
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

pub(crate) struct LayoutFrame {
    parent: Option<NodeId>,
    base: usize,
}

struct PlacementWatcher {
    read: ::reactive::ReadSignal<Rect>,
    write: ::reactive::WriteSignal<Rect>,
}

impl Document {
    pub fn new() -> Self {
        let theme = ThemeStore::new(Theme::DARK);
        Self {
            arena: Arena::default(),
            root: None,
            focused: None,
            activated: None,
            activation_key: None,
            rects: Rc::new(Rects::default()),
            inspector: None,
            inspectable: true,
            inspector_requested: false,
            screen_pointer: None,
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
            reattached: Cell::new(false),
            shortcuts: RefCell::new(Vec::new()),
            finger_taps: RefCell::new(Vec::new()),
            touch_scroll_vertical: None,
            touch_shift: crate::geometry::Vec2::ZERO,
            touch_scroll_horizontal: None,
            wheel_latch: None,
            autoscroll: None,
            pointer_capture: None,
            drags: Rc::default(),
            paste_requested: false,
            test_ids: HashMap::new(),
            node_test_ids: HashMap::new(),
            layout_revision: 0,
            paint_revision: 0,
            delivering: false,
            deferred_reveals: Vec::new(),
            constrained: HashSet::new(),
            measurements: NodeMap::default(),
            layout_parent: None,
            placed_children: NodeMap::default(),
            placing: Vec::new(),
            interact_pool: Vec::new(),
            engaged: Vec::new(),
            interact_parents: NodeMap::default(),
            interact_bounds: NodeMap::default(),
            interact_bounds_version: None,
            placed_pass: NodeMap::default(),
            reached_pass: NodeMap::default(),
            layout_pass: 0,
            scroll_hosts: Vec::new(),
            scroll_shifts: NodeMap::default(),
            viewport: None,
            shapes: Vec::new(),
            paint_cache: RefCell::new(PaintCache::default()),
            verifies_paint: true,
            copied_text: None,
            next_paint: None,
            reactive_scope: ::reactive::Scope::new(),
            theme,
            node_scopes: HashMap::new(),
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

    pub(crate) fn reactive_scope(&self) -> &::reactive::Scope {
        &self.reactive_scope
    }

    pub fn theme(&self) -> Theme {
        ::reactive::untrack(|| self.theme.get())
    }

    pub fn set_theme(&mut self, theme: Theme) {
        let store = self.theme.clone();
        crate::reactive::with_reactive_scope(self, move || store.set(theme));
    }

    pub fn open_inspector(&mut self) {
        self.inspector_requested = self.inspectable;
    }

    pub(crate) fn theme_store(&self) -> ThemeStore {
        self.theme.clone()
    }

    pub(crate) fn context(&self) -> Option<&Context> {
        self.viewport.as_ref().map(|(context, _, _)| context)
    }

    pub fn request_repaint_after(&self, delay: std::time::Duration) {
        if let Some((ctx, _, _)) = &self.viewport {
            ctx.request_repaint_after(delay);
        }
    }

    pub(crate) fn register_timer(&self, timer: Weak<crate::timer::TimerState>) {
        self.timers.borrow_mut().push(timer);
    }

    pub(crate) fn watch_context(&self) -> ::reactive::ReadSignal<u64> {
        self.attached.0.clone()
    }

    pub(crate) fn watch_pixels_per_point(&self) -> ::reactive::ReadSignal<f32> {
        self.scale.0.clone()
    }

    pub(crate) fn watch_focus_visible(&self) -> ::reactive::ReadSignal<bool> {
        self.focus_visible.0.clone()
    }

    pub(crate) fn set_focus_visible(&self, visible: bool) {
        self.focus_visible.1.set(visible);
    }

    pub(crate) fn register_shortcut(&self, shortcut: Weak<Shortcut>) {
        self.shortcuts.borrow_mut().push(shortcut);
    }

    pub(crate) fn register_finger_tap(&self, tap: Weak<FingerTap>) {
        self.finger_taps.borrow_mut().push(tap);
    }

    pub(crate) fn finger_tap(&self, fingers: usize) -> bool {
        let mut taps = self.finger_taps.borrow_mut();
        taps.retain(|tap| tap.strong_count() > 0);
        let live: Vec<Rc<FingerTap>> = taps.iter().filter_map(Weak::upgrade).collect();
        drop(taps);
        live.into_iter().any(|tap| tap(fingers))
    }

    pub(crate) fn key_shortcut(&self, press: KeyPress) -> bool {
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
        if self.reattached.replace(false) {
            self.attached.1.update(|attached| *attached += 1);
        }
        let now = Instant::now();
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

    pub(crate) fn register_node_scope(&mut self, node: NodeId, scope: ::reactive::Scope) {
        self.node_scopes.entry(node).or_default().push(scope);
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.arena.get(id).children()
    }

    pub(crate) fn open_child_slot<H: ChildHost>(&mut self, node: NodeId) -> SlotId {
        self.arena.get_mut_as::<H>(node).children().open()
    }

    pub(crate) fn fill_child_slot<H: ChildHost>(
        &mut self,
        node: NodeId,
        slot: SlotId,
        items: Vec<H::Stored>,
    ) {
        let host = self.arena.get_mut_as::<H>(node);
        host.children().fill_children(slot, items);
        host.children_changed();
    }

    pub(crate) fn append_child_item<H: ChildHost>(&mut self, node: NodeId, item: H::Stored) {
        self.arena.get_mut_as::<H>(node).children().push(item);
    }

    pub fn node_kind(&self, id: NodeId) -> &'static str {
        self.arena.get(id).kind()
    }

    pub fn node_detail(&self, id: NodeId) -> Option<String> {
        self.arena.get(id).detail()
    }

    pub fn node_rect(&self, id: NodeId) -> Option<Rect> {
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

    pub fn contains(&self, id: NodeId) -> bool {
        self.arena.contains(id)
    }

    pub fn performance(&self) -> PerformanceSnapshot {
        self.performance.snapshot()
    }

    pub fn reset_performance(&mut self) {
        self.performance.clear();
    }

    pub(crate) fn track_changes(&mut self, enabled: bool) {
        self.changes.set_enabled(enabled);
    }

    pub fn rubber_banding(&self) -> bool {
        self.rubber_banding
    }

    pub(crate) fn set_rubber_banding(&mut self, enabled: bool) {
        self.rubber_banding = enabled;
    }

    pub(crate) fn track_damage(&mut self, enabled: bool) {
        self.damage_flashes.set_enabled(enabled);
    }

    pub(crate) fn change_flashes(&self) -> impl Iterator<Item = (NodeId, Instant)> {
        self.changes.entries().map(|(id, at)| (*id, at))
    }

    pub(crate) fn damage_flashes(&self) -> impl Iterator<Item = (Rect, Instant)> {
        self.damage_flashes.entries().map(|(rect, at)| (*rect, at))
    }

    pub(crate) fn flashing(&self) -> bool {
        !self.changes.is_empty() || !self.damage_flashes.is_empty()
    }

    pub fn set_accessibility(&mut self, id: NodeId, node: Node) {
        if self.accessibility.get(&id) != Some(&node) {
            self.accessibility.insert(id, node);
            self.arena.invalidate_node(id);
        }
    }

    pub(crate) fn copy_text(&mut self, text: impl Into<String>) {
        self.copied_text = Some(text.into());
    }

    pub(crate) fn request_paste(&mut self) {
        self.paste_requested = true;
    }

    pub fn remove_node(&mut self, id: NodeId) {
        if !self.delivering {
            self.forget_placement(id);
        }
        let mut scopes = Vec::new();
        self.detach_subtree(id, &mut scopes);
        drop(scopes);
    }

    fn detach_subtree(&mut self, id: NodeId, scopes: &mut Vec<::reactive::Scope>) {
        if !self.arena.contains(id) {
            return;
        }
        let element = self.arena.get(id);
        let borrowed = element.borrowed();
        let children = element.children();
        self.release_portal(id, &borrowed);
        for child in children {
            if borrowed.contains(&child) {
                continue;
            }
            self.detach_subtree(child, scopes);
        }
        self.arena.remove(id);
        self.overlay_stack.retain(|overlay| *overlay != id);
        self.passive_overlays.retain(|overlay| *overlay != id);
        self.back_handlers.retain(|handler| *handler != id);
        self.paint_cache.borrow_mut().forget(id);
        self.sizes.remove(&id);
        self.placements.remove(&id);
        self.placed.remove(&id);
        self.measurements.remove(&id);
        self.component_states.remove(&id);
        self.component_names.remove(&id);
        self.placed_children.remove(&id);
        self.placed_pass.remove(&id);
        self.reached_pass.remove(&id);
        self.scroll_shifts.remove(&id);
        self.accessibility.remove(&id);
        self.accessibility_tree.get_mut().forget(id, &self.arena);
        for test_id in self.node_test_ids.remove(&id).unwrap_or_default() {
            self.drop_test_id(id, &test_id);
        }
        scopes.extend(self.node_scopes.remove(&id).unwrap_or_default());
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
        if self.inspectable {
            let theme = self.theme();
            if chord_pressed(ctx, Key::I) {
                self.inspector = match self.inspector {
                    Some(_) => None,
                    None => Some(Box::new(Inspector::new(ctx, theme))),
                };
            }
            if chord_pressed(ctx, Key::C) {
                self.inspector
                    .get_or_insert_with(|| Box::new(Inspector::new(ctx, theme)))
                    .toggle_picking();
            }
            if chord_pressed(ctx, Key::M) {
                self.inspector
                    .get_or_insert_with(|| Box::new(Inspector::new(ctx, theme)))
                    .toggle_responsive();
            }
            if chord_pressed(ctx, Key::F)
                && let Some(inspector) = self.inspector.as_mut()
            {
                inspector.toggle_focus();
            }
        }

        if self.inspector.is_none() {
            self.track_changes(false);
            self.track_damage(false);
        }

        let viewport = rect;
        let rect = trim_bottom(rect, ctx.measure_mouse_simulation(viewport)).0;
        let mut layout = match &mut self.inspector {
            Some(inspector) => inspector.layout(ctx, rect),
            None => Layout::app(rect),
        };
        let reserved = match &self.inspector {
            Some(inspector) if layout.app_visible => inspector.readout_height(ctx),
            _ => 0.0,
        };
        (layout.content, layout.readout) = trim_bottom(layout.content, reserved);
        let intercepted = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.intercepts());
        let inspector_has_focus = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.has_focus());
        let keys = match &self.inspector {
            _ if inspector_has_focus => Keys::Ignored,
            Some(inspector) => inspector.keys(),
            None => Keys::All,
        };
        if layout.app_visible {
            self.show_screen(ctx, layout.content, !intercepted, keys);
        } else {
            self.viewport = None;
        }
        if std::mem::take(&mut self.inspector_requested) && self.inspector.is_none() {
            self.inspector = Some(Box::new(Inspector::new(ctx, self.theme())));
            ctx.request_repaint();
        }

        if let Some(mut inspector) = self.inspector.take() {
            inspector.show(self, ctx, &layout, inspector_has_focus);
            if !inspector.closed() {
                self.inspector = Some(inspector);
            }
        }
        ctx.show_mouse_simulation(viewport);
    }

    fn show_screen(&mut self, ctx: &Context, rect: Rect, pointer: bool, keys: Keys) {
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
                        self.show_content(ctx, placement.screen, pointer, keys);
                    });
                });
            }
            None => self.show_content(ctx, rect, pointer, keys),
        }
    }

    pub(crate) fn screen_scale(&self) -> f32 {
        self.shown_screen().map_or(1.0, |placement| placement.scale)
    }

    pub(crate) fn shown_screen(&self) -> Option<Placement> {
        self.placement.and_then(|(_, placement)| placement)
    }

    pub(crate) fn show_content(&mut self, ctx: &Context, rect: Rect, pointer: bool, keys: Keys) {
        let mut measurement = FrameMeasurement::new();
        self.work.reset();
        let scale = ctx.pixels_per_point();
        if self
            .viewport
            .as_ref()
            .is_none_or(|(old_ctx, old_rect, old_scale)| {
                !ctx.same(old_ctx) || *old_rect != rect || *old_scale != scale
            })
        {
            let reattached = self
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
                let _guard = crate::reactive::install(self);
                context.run(|| {
                    crate::reactive::with_document(|document| {
                        for request in actions {
                            document.handle_accessibility_action(request);
                        }
                    });
                });
            }
        });

        {
            let context = self.reactive_scope().context();
            let _guard = crate::reactive::install(self);
            context.run(|| crate::reactive::with_document(|document| document.run_timers()));
        }

        if pointer || keys != Keys::Ignored {
            FrameMeasurement::measure(&mut measurement.timings.interaction, || {
                if let Some(root) = self.root {
                    let rects = Rc::clone(&self.rects);
                    let painter = ctx.painter();
                    let context = self.reactive_scope().context();
                    let _guard = crate::reactive::install(self);
                    context.run(|| {
                        crate::reactive::with_document(|document| {
                            interact::interact(document, ctx, &painter, &rects, root, pointer, keys)
                        });
                    });
                }
            });
        }
        if keys != Keys::Ignored
            && let Some(area) = self.focused_ime_area()
        {
            ctx.set_ime_area(Some(area));
        }
        if self.handles_back() {
            ctx.handle_back();
        }

        if let Some(text) = self.copied_text.take() {
            ctx.copy_text(text);
        }
        if std::mem::take(&mut self.paste_requested) {
            ctx.request_paste();
        }
        measurement.layout_passes =
            FrameMeasurement::measure(&mut measurement.timings.layout, || {
                usize::from(self.update_layout(ctx, rect))
            });
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
        let now = Instant::now();
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
            let previous = verifying.then(|| self.shapes.clone());
            FrameMeasurement::measure(&mut measurement.timings.paint, || self.paint(ctx));
            self.paint_revision = self.arena.revision;
            let region = self.damage.take(rect);
            if let Some(previous) = previous {
                self.verify_paint(ctx, rect, &previous, &region);
            }
            for damaged in region.rects() {
                self.damage_flashes.record(*damaged, now);
                ctx.report_damage(*damaged);
            }
            self.next_paint = self.paint_cache.get_mut().next_deadline();
        }
        if let Some(deadline) = self.next_paint {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
        if let Some(deadline) = self.next_timer() {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
        ctx.extend(&self.shapes);
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
        let frame = measurement.finish(self.arena.len(), self.shapes.len());
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
            if let Some(content) = self.overlay_content(overlay)
                && self.rects.contains_key(&content)
            {
                paint::paint(self, &painter, &self.rects, content);
                roots.push(content);
            }
        }
        let cache = self.paint_cache.get_mut();
        cache.settle_roots(roots);
        if cache.take_recorded() {
            self.shapes = cache.flatten();
        }
        self.damage.add_region(cache.take_damage());
    }

    fn verify_paint(
        &mut self,
        ctx: &Context,
        viewport: Rect,
        previous: &[Shape],
        region: &crate::damage::Region,
    ) {
        let counted = (
            self.work.painted_nodes.get(),
            self.work.replayed_nodes.get(),
        );
        let retained = self.paint_cache.replace(PaintCache::default());
        let shapes = std::mem::take(&mut self.shapes);
        let damage = std::mem::take(&mut self.damage);
        self.paint(ctx);
        let fresh = std::mem::replace(&mut self.shapes, shapes);
        self.paint_cache.replace(retained);
        self.damage = damage;
        self.work.painted_nodes.set(counted.0);
        self.work.replayed_nodes.set(counted.1);
        let differs = fresh
            .iter()
            .zip(&self.shapes)
            .position(|(fresh, retained)| !paint::same_shape(fresh, retained));
        assert!(
            fresh.len() == self.shapes.len() && differs.is_none(),
            "the retained painting differs from painting from scratch: {} shapes against {}, first at {differs:?}: {:?} against {:?}",
            fresh.len(),
            self.shapes.len(),
            differs.map(|at| crate::damage::bounds(&fresh[at])),
            differs.map(|at| crate::damage::bounds(&self.shapes[at])),
        );
        let prefix = previous
            .iter()
            .zip(&self.shapes)
            .take_while(|(old, new)| old == new)
            .count();
        let suffix = previous[prefix..]
            .iter()
            .rev()
            .zip(self.shapes[prefix..].iter().rev())
            .take_while(|(old, new)| old == new)
            .count();
        let old = &previous[prefix..previous.len() - suffix];
        let new = &self.shapes[prefix..self.shapes.len() - suffix];
        let moved = old
            .iter()
            .filter(|shape| !new.contains(shape))
            .chain(new.iter().filter(|shape| !old.contains(shape)));
        for shape in moved {
            let bounds = crate::damage::bounds(shape).intersect(viewport);
            assert!(
                !bounds.is_positive()
                    || region.rects().iter().any(|rect| rect.contains_rect(bounds)),
                "a shape changed outside the damaged region: {bounds:?} is not within {:?}",
                region.rects(),
            );
        }
    }

    pub(crate) fn watch_size(&mut self, id: NodeId) -> ::reactive::ReadSignal<Vec2> {
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

    pub(crate) fn watch_placement(&mut self, id: NodeId) -> ::reactive::ReadSignal<Rect> {
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

    pub(crate) fn register_placement_watcher(
        &mut self,
        id: NodeId,
        read: ::reactive::ReadSignal<Rect>,
        write: ::reactive::WriteSignal<Rect>,
    ) {
        self.placements
            .get_or_default(id)
            .push(PlacementWatcher { read, write });
    }

    pub(crate) fn register_size_watcher(
        &mut self,
        id: NodeId,
        read: ::reactive::ReadSignal<Vec2>,
        write: ::reactive::WriteSignal<Vec2>,
    ) {
        self.sizes
            .get_or_default(id)
            .push(SizeWatcher { read, write });
    }

    pub(crate) fn name_component(&mut self, id: NodeId, name: &'static str) {
        self.component_names.entry(id).or_default().push(name);
    }

    pub(crate) fn component_names(&self, id: NodeId) -> &[&'static str] {
        self.component_names.get(&id).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn set_component_state_dyn(&mut self, id: NodeId, state: Box<dyn Any>) {
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

    pub(crate) fn deliver_constraint(&mut self, id: NodeId, available: Vec2) {
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

    pub(crate) fn deliver_unmeasured_constraint(&mut self, id: NodeId, available: Vec2) {
        if self.constrained.contains(&id) {
            return;
        }
        self.deliver_constraint(id, available);
    }

    pub(crate) fn deliver_placement(&mut self, id: NodeId, rect: Rect) {
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

    pub(crate) fn settle_effects(&mut self) {
        if self.delivering {
            ::reactive::settle(|| {});
        }
    }

    pub(crate) fn assert_confined(&self, id: NodeId, watermark: usize) {
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

    pub(crate) fn measured(&mut self, id: NodeId, available: Vec2) -> Option<Vec2> {
        if self.arena.stale(id) {
            self.measurements.remove(&id);
            return None;
        }
        self.measurements
            .get(&id)?
            .iter()
            .find(|(offered, _)| same_size(*offered, available))
            .map(|(_, size)| *size)
    }

    pub(crate) fn remember_measurement(
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

    pub(crate) fn note_measured(&self, reused: bool) {
        let counter = match reused {
            true => &self.work.reused_measurements,
            false => &self.work.measured,
        };
        counter.set(counter.get() + 1);
    }

    pub(crate) fn note_placed_work(&self, reused: bool) {
        let counter = match reused {
            true => &self.work.reused_placements,
            false => &self.work.placed,
        };
        counter.set(counter.get() + 1);
    }

    pub(crate) fn note_painted(&self, reused: bool) {
        let counter = match reused {
            true => &self.work.replayed_nodes,
            false => &self.work.painted_nodes,
        };
        counter.set(counter.get() + 1);
    }

    pub(crate) fn note_parent(&mut self, id: NodeId) {
        if !self.delivering {
            return;
        }
        let parent = self.layout_parent;
        self.arena.set_parent(id, parent);
    }

    pub(crate) fn enter_measure(&mut self, id: NodeId) -> Option<NodeId> {
        self.layout_parent.replace(id)
    }

    pub(crate) fn leave_measure(&mut self, parent: Option<NodeId>) {
        self.layout_parent = parent;
    }

    pub(crate) fn enter_layout(&mut self, id: NodeId) -> LayoutFrame {
        let frame = LayoutFrame {
            parent: self.layout_parent,
            base: self.placing.len(),
        };
        self.layout_parent = Some(id);
        frame
    }

    pub(crate) fn leave_layout(&mut self, id: NodeId, frame: LayoutFrame, out: &Rects) {
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
        if out.remove(&id).is_some() {
            self.accessibility_tree.get_mut().mark(id, &self.arena);
            self.damage.add(self.paint_cache.borrow().bounds(id));
        }
        dropped.push(id);
        if self.delivering {
            self.deliver_placed(id, false);
        }
        for child in self.placed_children.remove(&id).unwrap_or_default() {
            self.drop_placement(child, out, dropped);
        }
    }

    fn forget_placement(&mut self, id: NodeId) {
        let rects = Rc::clone(&self.rects);
        self.drop_placement(id, &rects, &mut Vec::new());
    }

    pub(crate) fn take_interact_pool(&mut self) -> Vec<Vec<NodeId>> {
        std::mem::take(&mut self.interact_pool)
    }

    pub(crate) fn put_back_interact_pool(&mut self, pool: Vec<Vec<NodeId>>) {
        self.interact_pool = pool;
    }

    pub(crate) fn laying_out(&self) -> Option<NodeId> {
        self.layout_parent
    }

    pub(crate) fn delivering(&self) -> bool {
        self.delivering
    }

    pub(crate) fn invalidate_measurement(&mut self, id: NodeId) {
        self.arena.invalidate_node(id);
    }

    pub(crate) fn enter_scroll_host(&mut self, id: NodeId) {
        self.scroll_hosts.push(id);
    }

    pub(crate) fn leave_scroll_host(&mut self) {
        self.scroll_hosts.pop();
    }

    pub(crate) fn record_scroll_shift(&mut self, shift: f32) {
        let Some(&host) = self.scroll_hosts.last() else {
            return;
        };
        *self.scroll_shifts.get_or_default(host) += shift;
    }

    pub(crate) fn take_scroll_shift(&mut self, id: NodeId) -> f32 {
        self.scroll_shifts.remove(&id).unwrap_or(0.0)
    }

    pub(crate) fn placing_len(&self) -> usize {
        self.placing.len()
    }

    pub(crate) fn rewind_placing(&mut self, len: usize) {
        if self.delivering {
            self.placing.truncate(len);
        }
    }

    pub(crate) fn note_placed(&mut self, id: NodeId) {
        if self.delivering {
            self.placing.push(id);
            self.reached_pass.insert(id, self.layout_pass);
            self.deliver_placed(id, true);
        }
    }

    pub(crate) fn watch_placed(&mut self, id: NodeId) -> ::reactive::ReadSignal<bool> {
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

    pub(crate) fn reusable_placement(
        &self,
        id: NodeId,
        rect: Rect,
        painter: PainterState,
        out: &Rects,
    ) -> bool {
        self.delivering
            && !self.arena.unplaced(id)
            && out.get(&id) == Some(rect)
            && self.painters.get(&id).copied().map(PainterState::clip) == Some(painter.clip())
            && self.placed_children.contains_key(&id)
    }

    pub(crate) fn record_placement(
        &mut self,
        id: NodeId,
        rect: Rect,
        painter: PainterState,
        out: &Rects,
    ) {
        let previous = out.insert(id, rect);
        if !self.delivering {
            return;
        }
        self.arena.clear_unplaced(id);
        self.arena.note_relaid(id);
        self.placed_pass.insert(id, self.layout_pass);
        let previous_clip = self.painters.insert(id, painter).map(PainterState::clip);
        if previous != Some(rect) || previous_clip != Some(painter.clip()) {
            self.accessibility_tree.get_mut().mark(id, &self.arena);
        }
    }

    pub(crate) fn viewport_rect(&self) -> Rect {
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

    pub(crate) fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::new(self.pixels_per_point())
    }

    pub fn measure_root(&mut self, ctx: &Context, available: Vec2) -> Option<Vec2> {
        let root = self.root?;
        let painter = ctx.painter();
        let context = self.reactive_scope().context();
        let mut measured = None;
        {
            let _guard = crate::reactive::install(self);
            context.run(|| {
                crate::reactive::with_document(|document| {
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
                let _guard = crate::reactive::install(self);
                context.run(|| {
                    crate::reactive::with_document(|document| {
                        layout::layout(document, &painter, root, rect, placed);
                    });
                });
            }
            self.delivering = false;
        }
        self.placing.clear();
        self.lay_out_boundaries(ctx, &rects);
        self.layout_revision = self.arena.layout_revision;
        for node in std::mem::take(&mut self.deferred_reveals) {
            if self.arena.contains(node) {
                self.reveal_node(node);
            }
        }
        true
    }
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
            let (Some(rect), Some(&state)) = (rects.get(&boundary), self.painters.get(&boundary))
            else {
                continue;
            };
            let painter = Painter::resumed(ctx.clone(), state);
            let context = self.reactive_scope().context();
            self.layout_parent = self.arena.parent(boundary);
            self.delivering = true;
            {
                let _guard = crate::reactive::install(self);
                context.run(|| {
                    crate::reactive::with_document(|document| {
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

fn trim_bottom(rect: Rect, height: f32) -> (Rect, Rect) {
    let height = height.clamp(0.0, rect.height().max(0.0));
    let edge = rect.bottom() - height;
    (
        Rect::from_min_max(rect.min, pos2(rect.right(), edge)),
        Rect::from_min_max(pos2(rect.left(), edge), rect.max),
    )
}

fn chord_pressed(ctx: &Context, chord: Key) -> bool {
    ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if *key == chord && modifiers.ctrl && modifiers.shift
            )
        })
    })
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
