use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::time::Instant;

use accesskit::Node;

use crate::accessibility;
use crate::base::child_list::{ChildHost, SlotId};
use crate::context::Context;
use crate::damage::{Damage, Region};
use crate::flash::FlashLog;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::input::{Event, Key, KeyPress};

use crate::inspector::Inspector;
use crate::interact;
use crate::layout;
use crate::node::{Arena, NodeId, NodeMap};
use crate::paint::{self, PaintCache, Painted};
use crate::painter::Shape;
use crate::performance::{FrameMeasurement, FrameWork, PerformanceSnapshot, PerformanceTracker};
use crate::pixel_grid::PixelGrid;
use crate::styled::{Theme, ThemeStore};

pub(crate) type Shortcut = dyn Fn(KeyPress) -> bool;

pub struct Document {
    pub(crate) arena: Arena,
    pub(crate) root: Option<NodeId>,
    pub(crate) focused: Option<NodeId>,
    pub(crate) activated: Option<NodeId>,
    pub(crate) activation_key: Option<Key>,
    pub(crate) rects: Rc<NodeMap<Rect>>,
    pub(crate) inspector: Option<Box<Inspector>>,
    pub(crate) inspectable: bool,
    pub(crate) portal_holders: std::collections::HashMap<NodeId, NodeId>,
    pub(crate) overlay_stack: Vec<NodeId>,
    pub(crate) passive_overlays: Vec<NodeId>,
    frame_hooks: RefCell<Vec<Weak<dyn Fn()>>>,
    shortcuts: RefCell<Vec<Weak<Shortcut>>>,
    pub(crate) touch_scroll_vertical: Option<NodeId>,
    pub(crate) touch_scroll_horizontal: Option<NodeId>,
    pub(crate) pointer_capture: Option<NodeId>,
    paste_requested: bool,
    test_ids: HashMap<String, NodeId>,
    node_test_ids: HashMap<NodeId, Vec<String>>,
    layout_revision: u64,
    paint_revision: u64,
    delivering: bool,
    constrained: HashSet<NodeId>,
    measurements: NodeMap<Vec<(Vec2, Vec2)>>,
    layout_parent: Option<NodeId>,
    placed_children: NodeMap<Vec<NodeId>>,
    placing: Vec<NodeId>,
    interact_pool: Vec<Vec<NodeId>>,
    placed_pass: NodeMap<u64>,
    layout_pass: u64,
    scroll_hosts: Vec<NodeId>,
    scroll_shifts: NodeMap<f32>,
    viewport: Option<(Context, Rect, f32)>,
    shapes: Vec<Shape>,
    paint_cache: RefCell<PaintCache>,
    paint_region: Cell<Region>,
    paint_base: Cell<Option<usize>>,
    grown: Cell<Rect>,
    deadlines: Vec<NodeId>,
    pub(crate) copied_text: Option<String>,
    next_paint: Option<Instant>,
    reactive_scope: ::reactive::Scope,
    theme: ThemeStore,
    node_scopes: HashMap<NodeId, Vec<::reactive::Scope>>,
    sizes: NodeMap<Vec<SizeWatcher>>,
    placements: NodeMap<Vec<PlacementWatcher>>,
    component_states: HashMap<NodeId, Vec<Box<dyn Any>>>,
    pub(crate) accessibility_id: u32,
    pub(crate) accessibility: NodeMap<Node>,
    performance: PerformanceTracker,
    work: WorkCounters,
    changes: FlashLog<NodeId>,
    damage: Damage,
    damage_flashes: FlashLog<Rect>,
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
struct WorkCounters {
    measured: Cell<usize>,
    reused_measurements: Cell<usize>,
    placed: Cell<usize>,
    reused_placements: Cell<usize>,
    painted_nodes: Cell<usize>,
    replayed_nodes: Cell<usize>,
}

impl WorkCounters {
    fn reset(&self) {
        self.measured.set(0);
        self.reused_measurements.set(0);
        self.placed.set(0);
        self.reused_placements.set(0);
        self.painted_nodes.set(0);
        self.replayed_nodes.set(0);
    }

    fn gathered(&self) -> FrameWork {
        FrameWork {
            measured: self.measured.get(),
            reused_measurements: self.reused_measurements.get(),
            placed: self.placed.get(),
            reused_placements: self.reused_placements.get(),
            painted_nodes: self.painted_nodes.get(),
            replayed_nodes: self.replayed_nodes.get(),
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
            rects: Rc::new(NodeMap::default()),
            inspector: None,
            inspectable: true,
            portal_holders: std::collections::HashMap::new(),
            overlay_stack: Vec::new(),
            passive_overlays: Vec::new(),
            frame_hooks: RefCell::new(Vec::new()),
            shortcuts: RefCell::new(Vec::new()),
            touch_scroll_vertical: None,
            touch_scroll_horizontal: None,
            pointer_capture: None,
            paste_requested: false,
            test_ids: HashMap::new(),
            node_test_ids: HashMap::new(),
            layout_revision: 0,
            paint_revision: 0,
            delivering: false,
            constrained: HashSet::new(),
            measurements: NodeMap::default(),
            layout_parent: None,
            placed_children: NodeMap::default(),
            placing: Vec::new(),
            interact_pool: Vec::new(),
            placed_pass: NodeMap::default(),
            layout_pass: 0,
            scroll_hosts: Vec::new(),
            scroll_shifts: NodeMap::default(),
            viewport: None,
            shapes: Vec::new(),
            paint_cache: RefCell::new(PaintCache::default()),
            paint_region: Cell::new(Region::NOTHING),
            paint_base: Cell::new(None),
            grown: Cell::new(Rect::NOTHING),
            deadlines: Vec::new(),
            copied_text: None,
            next_paint: None,
            reactive_scope: ::reactive::Scope::new(),
            theme,
            node_scopes: HashMap::new(),
            sizes: NodeMap::default(),
            placements: NodeMap::default(),
            component_states: HashMap::new(),
            accessibility_id: accessibility::next_document_id(),
            accessibility: NodeMap::default(),
            performance: PerformanceTracker::default(),
            work: WorkCounters::default(),
            changes: FlashLog::default(),
            damage: Damage::default(),
            damage_flashes: FlashLog::default(),
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

    pub(crate) fn register_frame_hook(&self, work: Weak<dyn Fn()>) {
        self.frame_hooks.borrow_mut().push(work);
    }

    pub(crate) fn register_shortcut(&self, shortcut: Weak<Shortcut>) {
        self.shortcuts.borrow_mut().push(shortcut);
    }

    pub(crate) fn key_shortcut(&self, press: KeyPress) -> bool {
        let mut shortcuts = self.shortcuts.borrow_mut();
        shortcuts.retain(|shortcut| shortcut.strong_count() > 0);
        let live: Vec<Rc<Shortcut>> = shortcuts.iter().filter_map(Weak::upgrade).collect();
        drop(shortcuts);
        live.into_iter().any(|shortcut| shortcut(press))
    }

    fn run_frame_hooks(&self) {
        let mut hooks = self.frame_hooks.borrow_mut();
        hooks.retain(|hook| hook.strong_count() > 0);
        let live: Vec<Rc<dyn Fn()>> = hooks.iter().filter_map(Weak::upgrade).collect();
        drop(hooks);
        for hook in live {
            hook();
        }
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
        self.rects.get(&id).copied()
    }

    pub fn set_test_id(&mut self, id: NodeId, test_id: impl Into<String>) {
        let test_id = test_id.into();
        if test_id.is_empty() {
            return;
        }
        if let Some(previous) = self.test_ids.insert(test_id.clone(), id)
            && previous != id
        {
            self.forget_test_id(previous, &test_id);
        }
        let owned = self.node_test_ids.entry(id).or_default();
        if !owned.iter().any(|existing| existing == &test_id) {
            owned.push(test_id);
        }
    }

    pub fn clear_test_id(&mut self, id: NodeId, test_id: &str) {
        self.forget_test_id(id, test_id);
        if self.test_ids.get(test_id) == Some(&id) {
            self.test_ids.remove(test_id);
        }
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
        self.test_ids.get(test_id).copied()
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

    pub(crate) fn track_damage(&mut self, enabled: bool) {
        self.damage_flashes.set_enabled(enabled);
    }

    pub(crate) fn paint_cache(&self) -> std::cell::Ref<'_, PaintCache> {
        self.paint_cache.borrow()
    }

    pub(crate) fn cached_start(&self, id: NodeId, parent: Option<NodeId>) -> Option<usize> {
        self.paint_cache.borrow().start(id, parent)
    }

    pub(crate) fn cached_bounds(&self, id: NodeId) -> Rect {
        self.paint_cache.borrow().bounds(id)
    }

    pub(crate) fn store_painted(&self, id: NodeId, painted: Painted) {
        self.paint_cache.borrow_mut().store(id, painted);
    }

    pub(crate) fn paint_region(&self) -> Region {
        self.paint_region.get()
    }

    pub(crate) fn paint_base(&self) -> Option<usize> {
        self.paint_base.get()
    }

    pub(crate) fn enter_paint_base(&self, base: Option<usize>) -> Option<usize> {
        self.paint_base.replace(base)
    }

    pub(crate) fn leave_paint_base(&self, base: Option<usize>) {
        self.paint_base.set(base);
    }

    pub(crate) fn painted_shapes(&self, base: usize, len: usize) -> Option<&[Shape]> {
        self.shapes.get(base..base + len)
    }

    pub(crate) fn note_grown(&self, bounds: Rect) {
        self.grown.set(self.grown.get().union(bounds));
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
        self.paint_cache.borrow_mut().forget(id);
        self.sizes.remove(&id);
        self.placements.remove(&id);
        self.measurements.remove(&id);
        self.component_states.remove(&id);
        self.placed_children.remove(&id);
        self.scroll_shifts.remove(&id);
        self.accessibility.remove(&id);
        for test_id in self.node_test_ids.remove(&id).unwrap_or_default() {
            if self.test_ids.get(&test_id) == Some(&id) {
                self.test_ids.remove(&test_id);
            }
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
        let (content, panel) = match &mut self.inspector {
            Some(inspector) => {
                inspector.grab(ctx, rect);
                split(rect, inspector.panel_width(ctx, rect))
            }
            None => (rect, Rect::NOTHING),
        };
        let reserved = self
            .inspector
            .as_ref()
            .map_or(0.0, |inspector| inspector.readout_height(ctx));
        let (content, readout) = trim_bottom(content, reserved);
        let intercepted = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.intercepts());
        let inspector_has_focus = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.document.focused_node().is_some());
        self.show_content(ctx, content, !intercepted, !inspector_has_focus);

        if let Some(mut inspector) = self.inspector.take() {
            inspector.show(self, ctx, content, readout, panel, inspector_has_focus);
            if !inspector.closed() {
                self.inspector = Some(inspector);
            }
        }
        ctx.show_mouse_simulation(viewport);
    }

    pub(crate) fn show_content(
        &mut self,
        ctx: &Context,
        rect: Rect,
        interactive: bool,
        keyboard_interactive: bool,
    ) {
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
            context.run(|| crate::reactive::with_document(|document| document.run_frame_hooks()));
        }

        if interactive {
            FrameMeasurement::measure(&mut measurement.timings.interaction, || {
                if let Some(root) = self.root {
                    let rects = Rc::clone(&self.rects);
                    let painter = ctx.painter();
                    let context = self.reactive_scope().context();
                    let _guard = crate::reactive::install(self);
                    context.run(|| {
                        crate::reactive::with_document(|document| {
                            interact::interact(
                                document,
                                ctx,
                                &painter,
                                &rects,
                                root,
                                keyboard_interactive,
                            )
                        });
                    });
                }
            });
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
            for (test_id, id) in &self.test_ids {
                if let Some(node_rect) = self.rects.get(id) {
                    ctx.publish_test_id(test_id, *node_rect);
                }
            }
        }
        let now = Instant::now();
        self.changes.prune(now);
        self.damage_flashes.prune(now);
        if self.arena.take_everything() {
            self.damage.everything();
        }
        for id in self.arena.take_changed() {
            self.changes.record(id, now);
            if let Some(node) = self.rects.get(&id)
                && self.paints(id)
            {
                self.damage.add(*node);
            }
            self.damage.add(self.paint_cache.borrow().bounds(id));
        }
        let due = self.next_paint.is_some_and(|deadline| deadline <= now);
        if due {
            for id in std::mem::take(&mut self.deadlines) {
                self.damage.add(self.paint_cache.borrow().bounds(id));
            }
        }
        if self.paint_revision != self.arena.revision || due {
            measurement.painted = true;
            self.paint_region.set(self.damage.take(rect));
            self.grown.set(Rect::NOTHING);
            let (shapes, delay) = FrameMeasurement::measure(&mut measurement.timings.paint, || {
                ctx.capture(|| {
                    if let Some(root) = self.root {
                        self.paint_base.set(Some(0));
                        paint::paint(self, &ctx.painter(), &self.rects, root);
                    }
                    ctx.flush_top();
                    let overlays: Vec<NodeId> = self
                        .overlay_stack
                        .iter()
                        .chain(self.passive_overlays.iter())
                        .copied()
                        .collect();
                    for overlay in overlays {
                        if let Some(content) = self.overlay_content(overlay)
                            && self.rects.contains_key(&content)
                        {
                            self.paint_base.set(Some(0));
                            paint::paint(self, &ctx.painter(), &self.rects, content);
                        }
                        ctx.flush_top();
                    }
                })
            });
            self.shapes = shapes;
            self.deadlines = ctx.take_deadlines();
            let mut region = self.paint_region.get();
            region.add(self.grown.get());
            for damaged in region.rects() {
                let damaged = damaged.intersect(rect);
                if damaged.is_positive() {
                    self.damage_flashes.record(damaged, now);
                    ctx.report_damage(damaged);
                }
            }
            self.next_paint = Instant::now().checked_add(delay);
            self.paint_revision = self.arena.revision;
        }
        if let Some(deadline) = self.next_paint {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
        ctx.extend(&self.shapes);
        FrameMeasurement::measure(&mut measurement.timings.accessibility, || {
            if !ctx.accessibility_active() {
                return;
            }
            if let Some(fragment) = self.accessibility_fragment() {
                ctx.publish_accessibility(fragment);
            }
        });
        measurement.work = self.work.gathered();
        let frame = measurement.finish(self.arena.len(), self.shapes.len());
        self.performance.record(frame);
    }

    pub(crate) fn watch_size(&mut self, id: NodeId) -> ::reactive::ReadSignal<Vec2> {
        if let Some(watcher) = self.sizes.get(&id).and_then(|watchers| watchers.first()) {
            return watcher.read.clone();
        }
        let size = self.rects.get(&id).map_or(Vec2::ZERO, Rect::size);
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
        let rect = self.rects.get(&id).copied().unwrap_or(Rect::ZERO);
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
            for changed in self.arena.changed_since(watermark) {
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
        if self.arena.revision != watermark {
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

    pub(crate) fn leave_layout(&mut self, id: NodeId, frame: LayoutFrame, out: &mut NodeMap<Rect>) {
        self.layout_parent = frame.parent;
        if self.delivering {
            for child in self.dropped_children(id, frame.base) {
                self.drop_placement(child, out);
            }
        }
        self.placing.truncate(frame.base);
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

    fn drop_placement(&mut self, id: NodeId, out: &mut NodeMap<Rect>) {
        if let Some(rect) = out.remove(&id) {
            if self.paints(id) {
                self.damage.add(rect);
            }
            self.damage.add(self.paint_cache.borrow().bounds(id));
        }
        for child in self.placed_children.remove(&id).unwrap_or_default() {
            self.drop_placement(child, out);
        }
    }

    fn forget_placement(&mut self, id: NodeId) {
        let mut rects = std::mem::take(&mut self.rects);
        self.drop_placement(id, Rc::make_mut(&mut rects));
        self.rects = rects;
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
        }
    }

    pub(crate) fn reusable_placement(&self, id: NodeId, rect: Rect, out: &NodeMap<Rect>) -> bool {
        self.delivering
            && !self.arena.unplaced(id)
            && out.get(&id) == Some(&rect)
            && self.placed_children.contains_key(&id)
    }

    pub(crate) fn record_placement(&mut self, id: NodeId, rect: Rect, out: &mut NodeMap<Rect>) {
        let previous = out.insert(id, rect);
        if !self.delivering {
            return;
        }
        self.arena.clear_unplaced(id);
        self.placed_pass.insert(id, self.layout_pass);
        if previous == Some(rect) {
            return;
        }
        if self.paints(id) {
            self.damage.add(rect);
            if let Some(previous) = previous {
                self.damage.add(previous);
            }
        }
        self.damage.add(self.paint_cache.borrow().bounds(id));
    }

    fn paints(&self, id: NodeId) -> bool {
        match self.arena.contains(id) {
            true => self.arena.get(id).paints(),
            false => true,
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

    fn update_layout(&mut self, ctx: &Context, rect: Rect) -> bool {
        if self.layout_revision == self.arena.revision {
            return false;
        }
        self.layout_pass = self.layout_pass.wrapping_add(1);
        let mut rects = (*self.rects).clone();
        self.placing.clear();
        if let Some(root) = self.root {
            let painter = ctx.painter();
            let context = self.reactive_scope().context();
            let placed = &mut rects;
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
        self.rects = Rc::new(rects);
        self.placing.clear();
        self.layout_revision = self.arena.revision;
        true
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

fn split(rect: Rect, width: f32) -> (Rect, Rect) {
    let edge = rect.right() - width;
    (
        Rect::from_min_max(rect.min, pos2(edge, rect.bottom())),
        Rect::from_min_max(pos2(edge, rect.top()), rect.max),
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
