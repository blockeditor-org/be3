extern crate beui_view as beui;

pub mod mouse_simulation;
pub mod overlay;
pub mod panel;
pub mod responsive;
pub mod screen_reader;
pub mod tree;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use beui_core::context::Context;
use beui_core::filter::{ColorVision, Filter};
use beui_core::flash;
use beui_core::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use beui_core::input::{CursorIcon, Event, Key as InputKey};
use beui_core::interact::Keys;
use beui_core::painter::Painter;

use crate::screen_reader::{Command, ScreenReader};
use beui_components_styled::{DocumentTheme, Theme};
use beui_core::document::{Document, Tools, trim_bottom};
use beui_core::node::NodeId;
use beui_core::screen_simulation::ScreenSimulation;
use beui_view::reactive::{NodeRef, WriteSignal, with_document, with_reactive_scope};

use panel::Summary;
use tree::{Entry, Key};

const DEFAULT_WIDTH: f32 = 340.0;
const MINIMUM_WIDTH: f32 = 200.0;
const GRIP_WIDTH: f32 = 4.0;
const GRIP_PAINT_WIDTH: f32 = 2.0;
const MINIMUM_APP_WIDTH: f32 = 480.0;
const NARROWEST_APP_WIDTH: f32 = 200.0;
const BAR_HEIGHT: f32 = 44.0;
const HANDLE_REACH: f32 = beui_core::screen_simulation::MARGIN;
const HANDLE_THICKNESS: f32 = 4.0;
const HANDLE_LENGTH: f32 = 40.0;
const HANDLE_CORNER: f32 = 14.0;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum InspectorTab {
    #[default]
    Beui,
    Components,
    AccessKit,
    Performance,
    Simulation,
}

impl InspectorTab {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Components,
            2 => Self::AccessKit,
            3 => Self::Performance,
            4 => Self::Simulation,
            _ => Self::Beui,
        }
    }
}

pub struct State {
    expansion: RefCell<HashMap<Key, bool>>,
    tab: Cell<InspectorTab>,
    pub hovered: Cell<Option<NodeId>>,
    pub selected: Cell<Option<NodeId>>,
    selected_component: Cell<Option<Key>>,
    pub picking: Cell<bool>,
    pub touch_emulation: Cell<bool>,
    pub mouse_simulation: Cell<bool>,
    pub rubber_banding: Cell<bool>,
    pub flash_changes: Cell<bool>,
    pub flash_damage: Cell<bool>,
    pub simulated_pixels_per_point: Cell<Option<f32>>,
    pub screen_simulation: Cell<Option<ScreenSimulation>>,
    pub screen_reader: Cell<bool>,
    pub accessibility: Cell<bool>,
    pub blur: Cell<f32>,
    pub contrast_reduction: Cell<f32>,
    pub color_vision: Cell<ColorVision>,
    commands: RefCell<Vec<Command>>,
    pub theme: Cell<Theme>,
    requested_theme: Cell<Option<Theme>>,
    revision: Cell<u64>,
    reset_performance: Cell<bool>,
    closed: Cell<bool>,
    compact: Cell<bool>,
    app_shown: Cell<bool>,
}

impl State {
    fn new(ctx: &Context, theme: Theme) -> Self {
        Self {
            expansion: RefCell::new(HashMap::new()),
            tab: Cell::new(InspectorTab::default()),
            hovered: Cell::new(None),
            selected: Cell::new(None),
            selected_component: Cell::new(None),
            picking: Cell::new(false),
            touch_emulation: Cell::new(ctx.touch_emulation()),
            mouse_simulation: Cell::new(ctx.mouse_simulation()),
            rubber_banding: Cell::new(true),
            flash_changes: Cell::new(false),
            flash_damage: Cell::new(false),
            simulated_pixels_per_point: Cell::new(ctx.simulated_pixels_per_point()),
            screen_simulation: Cell::new(ctx.screen_simulation()),
            screen_reader: Cell::new(false),
            accessibility: Cell::new(ctx.accessibility_active()),
            blur: Cell::new(0.0),
            contrast_reduction: Cell::new(0.0),
            color_vision: Cell::new(ColorVision::Typical),
            commands: RefCell::new(Vec::new()),
            theme: Cell::new(theme),
            requested_theme: Cell::new(None),
            revision: Cell::new(0),
            reset_performance: Cell::new(false),
            closed: Cell::new(false),
            compact: Cell::new(false),
            app_shown: Cell::new(false),
        }
    }

    fn expanded(&self, key: Key, default: bool) -> bool {
        self.expansion
            .borrow()
            .get(&key)
            .copied()
            .unwrap_or(default)
    }

    fn set_expanded(&self, key: Key, expanded: bool) {
        self.expansion.borrow_mut().insert(key, expanded);
        self.touch();
    }

    fn expand_ancestors(&self, ancestors: &[Key]) {
        let mut expansion = self.expansion.borrow_mut();
        for key in ancestors {
            expansion.insert(*key, true);
        }
        drop(expansion);
        self.touch();
    }

    fn set_tab(&self, index: usize) {
        self.tab.set(InspectorTab::from_index(index));
        self.touch();
    }

    fn reset_performance(&self) {
        self.reset_performance.set(true);
        self.touch();
    }

    fn close(&self) {
        self.closed.set(true);
        self.touch();
    }

    fn simulate_screen(&self, simulation: Option<ScreenSimulation>) {
        if self.screen_simulation.replace(simulation) != simulation {
            self.touch();
        }
    }

    fn update_screen(&self, change: impl FnOnce(ScreenSimulation) -> ScreenSimulation) {
        if let Some(simulation) = self.screen_simulation.get() {
            self.simulate_screen(Some(change(simulation)));
        }
    }

    fn toggle_responsive(&self) {
        let simulation = match self.screen_simulation.get() {
            Some(_) => None,
            None => Some(responsive::DEFAULT),
        };
        self.simulate_screen(simulation);
    }

    fn simulate_pixels_per_point(&self, pixels_per_point: Option<f32>) {
        self.simulated_pixels_per_point.set(pixels_per_point);
        self.touch();
    }

    fn enable_accessibility(&self, enabled: bool) {
        self.accessibility.set(enabled);
        self.touch();
    }

    fn enable_screen_reader(&self, enabled: bool) {
        self.screen_reader.set(enabled);
        self.touch();
    }

    fn set_blur(&self, radius: f32) {
        self.blur.set(radius);
        self.touch();
    }

    fn set_contrast_reduction(&self, amount: f32) {
        self.contrast_reduction.set(amount);
        self.touch();
    }

    fn choose_color_vision(&self, vision: ColorVision) {
        self.color_vision.set(vision);
        self.touch();
    }

    fn filter(&self, region: Rect) -> Filter {
        Filter {
            region,
            blur: self.blur.get().max(0.0),
            contrast: 1.0 - self.contrast_reduction.get().clamp(0.0, 1.0),
            vision: self.color_vision.get(),
        }
    }

    fn command(&self, command: Command) {
        self.commands.borrow_mut().push(command);
        self.touch();
    }

    fn take_commands(&self) -> Vec<Command> {
        std::mem::take(&mut self.commands.borrow_mut())
    }

    fn choose_theme(&self, theme: Theme) {
        self.theme.set(theme);
        self.requested_theme.set(Some(theme));
        self.touch();
    }

    fn hover(&self, id: NodeId, hovered: bool) {
        match hovered {
            true => self.hovered.set(Some(id)),
            false if self.hovered.get() == Some(id) => self.hovered.set(None),
            false => {}
        }
    }

    fn select(&self, id: NodeId) {
        self.selected.set(Some(id));
        self.selected_component.set(None);
        self.touch();
    }

    fn select_row(&self, key: Key) {
        self.select(key.node());
        if let Key::Component(..) = key {
            self.selected_component.set(Some(key));
        }
    }

    fn toggle_picking(&self) {
        self.picking.set(!self.picking.get());
        if self.picking.get() {
            self.app_shown.set(true);
        }
        self.touch();
    }

    fn show_app(&self, shown: bool) {
        self.app_shown.set(shown);
        self.touch();
    }

    pub fn app_visible(&self) -> bool {
        !self.compact.get() || self.app_shown.get()
    }

    fn touch(&self) {
        self.revision.set(self.revision.get() + 1);
    }
}

pub struct Layout {
    pub bar: Rect,
    pub toolbar: Rect,
    pub content: Rect,
    pub readout: Rect,
    pub panel: Rect,
    pub app_visible: bool,
}

impl Layout {
    pub fn app(rect: Rect) -> Self {
        Self {
            bar: Rect::NOTHING,
            toolbar: Rect::NOTHING,
            content: rect,
            readout: Rect::NOTHING,
            panel: Rect::NOTHING,
            app_visible: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Handle {
    Right,
    Bottom,
    Corner,
}

impl Handle {
    fn cursor(self) -> CursorIcon {
        match self {
            Handle::Right => CursorIcon::ResizeHorizontal,
            Handle::Bottom => CursorIcon::ResizeVertical,
            Handle::Corner => CursorIcon::ResizeNwSe,
        }
    }

    fn resized(self, size: Vec2, moved: Vec2) -> Vec2 {
        let width = size.x + 2.0 * moved.x;
        let height = size.y + moved.y;
        match self {
            Handle::Right => vec2(width, size.y),
            Handle::Bottom => vec2(size.x, height),
            Handle::Corner => vec2(width, height),
        }
    }
}

#[derive(Clone, Copy)]
struct Resize {
    handle: Handle,
    origin: Pos2,
    size: Vec2,
    scale: f32,
}

#[derive(Clone, Copy)]
enum Layer {
    Below(f32),
    Above,
}

pub struct Inspector {
    pub document: Document,
    pub entries: Vec<Entry>,
    pub state: Rc<State>,
    reader: ScreenReader,
    set_keys: WriteSignal<Vec<Key>>,
    set_entries: WriteSignal<HashMap<Key, Entry>>,
    set_summary: WriteSignal<Summary>,
    set_performance: WriteSignal<panel::PerformanceSummary>,
    set_renderer: WriteSignal<panel::RendererRows>,
    set_selection: WriteSignal<Vec<Key>>,
    tree: NodeRef,
    bar: panel::Bar,
    toolbar: responsive::Toolbar,
    resizing: Option<Resize>,
    handle: Option<Handle>,
    handle_bounds: Cell<Rect>,
    toolbar_area: Rect,
    pub width: f32,
    grabbed: Option<f32>,
    grip: bool,
    parked: bool,
    seen: u64,
    overlay_bounds: Cell<Rect>,
}

impl Inspector {
    pub fn new(ctx: &Context, theme: Theme) -> Self {
        let state = Rc::new(State::new(ctx, theme));
        let panel = panel::build(&state);
        let bar = panel::build_bar(&state);
        let toolbar = responsive::build(&state);
        Self {
            bar,
            toolbar,
            resizing: None,
            handle: None,
            handle_bounds: Cell::new(Rect::NOTHING),
            toolbar_area: Rect::NOTHING,
            document: panel.document,
            entries: Vec::new(),
            tree: panel.tree,
            state,
            reader: ScreenReader::default(),
            set_keys: panel.set_keys,
            set_entries: panel.set_entries,
            set_summary: panel.set_summary,
            set_performance: panel.set_performance,
            set_renderer: panel.set_renderer,
            set_selection: panel.set_selection,
            width: DEFAULT_WIDTH,
            grabbed: None,
            grip: false,
            parked: false,
            seen: 0,
            overlay_bounds: Cell::new(Rect::NOTHING),
        }
    }

    pub fn row_node(&self, index: usize) -> NodeId {
        self.find(&self.entries[index].key.test_id())
    }

    pub fn find(&self, test_id: &str) -> NodeId {
        self.document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("the inspector has no node with test id {test_id:?}"))
    }

    pub fn option_node(&self, test_id: &str, index: usize) -> NodeId {
        self.document.children(self.find(test_id))[index]
    }

    pub fn bar_option_rect(&self, index: usize) -> Option<Rect> {
        let document = &self.bar.document;
        let bar = document.find_test_id("inspector.bar")?;
        document.node_rect(document.children(bar)[index])
    }

    pub fn toolbar_rect(&self, test_id: &str) -> Option<Rect> {
        let document = &self.toolbar.document;
        document.node_rect(document.find_test_id(test_id)?)
    }

    pub fn panel_rect(&self) -> Option<Rect> {
        self.document
            .root()
            .and_then(|root| self.document.node_rect(root))
    }

    pub fn reader(&self) -> &ScreenReader {
        &self.reader
    }

    pub fn selected_row(&self, target: &Document) -> Option<String> {
        let key = self.selection_path(target).last().copied()?;
        let entry = self.entries.iter().find(|entry| entry.key == key)?;
        Some(entry.kind.clone())
    }

    pub fn focused_row(&self) -> Option<Key> {
        beui_components_styled::tree_focused::<Key>(&self.document, self.tree.get())
    }

    pub fn panel_width(&self, ctx: &Context, rect: Rect) -> f32 {
        let scale = scale(ctx);
        (self.width * scale)
            .min(rect.width() - NARROWEST_APP_WIDTH * scale)
            .max(0.0)
    }

    pub fn closed(&self) -> bool {
        self.state.closed.get()
    }

    pub fn readout_height(&self, ctx: &Context) -> f32 {
        match self.state.screen_reader.get() {
            true => ScreenReader::height() * scale(ctx),
            false => 0.0,
        }
    }

    pub fn layout(&mut self, ctx: &Context, rect: Rect) -> Layout {
        ctx.set_screen_simulation(self.screen_simulation());
        let scale = scale(ctx);
        let compact = rect.width() < (MINIMUM_APP_WIDTH + DEFAULT_WIDTH) * scale;
        self.state.compact.set(compact);
        let layout = match compact {
            false => {
                self.grab(ctx, rect);
                let (content, panel) = split(rect, self.panel_width(ctx, rect));
                Layout {
                    panel,
                    ..Layout::app(content)
                }
            }
            true => {
                self.grabbed = None;
                self.grip = false;
                let (bar, rect) = trim_top(rect, BAR_HEIGHT * scale);
                let app_visible = self.state.app_visible();
                Layout {
                    bar,
                    toolbar: Rect::NOTHING,
                    content: rect,
                    readout: Rect::NOTHING,
                    panel: if app_visible { Rect::NOTHING } else { rect },
                    app_visible,
                }
            }
        };
        if !layout.app_visible || self.state.screen_simulation.get().is_none() {
            return layout;
        }
        let (toolbar, content) = trim_top(layout.content, responsive::HEIGHT * scale);
        Layout {
            toolbar,
            content,
            ..layout
        }
    }

    pub fn has_focus(&self) -> bool {
        self.document.focused_node().is_some() || self.toolbar.document.focused_node().is_some()
    }

    pub fn intercepts(&self) -> bool {
        self.state.picking.get()
            || self.grabbed.is_some()
            || self.resizing.is_some()
            || self.toolbar.open()
            || self.state.screen_reader.get()
    }

    pub fn keys(&self) -> Keys {
        if self.state.picking.get() || self.grabbed.is_some() || self.resizing.is_some() {
            return Keys::Ignored;
        }
        match self.state.screen_reader.get() {
            true => Keys::Except(crate::screen_reader::claims),
            false => Keys::All,
        }
    }

    pub fn toggle_picking(&self) {
        self.state.toggle_picking();
    }

    pub fn toggle_responsive(&self) {
        self.state.toggle_responsive();
    }

    fn grab(&mut self, ctx: &Context, rect: Rect) {
        let scale = scale(ctx);
        let edge = rect.right() - self.panel_width(ctx, rect);
        let grip = Rect::from_min_max(
            pos2(edge - GRIP_WIDTH, rect.top()),
            pos2(edge + GRIP_WIDTH, rect.bottom()),
        );
        if ctx.input(|input| input.pointer.primary_released()) {
            self.grabbed = None;
        }
        let Some(pointer) = ctx.input(|input| input.pointer.interact_pos()) else {
            self.grip = false;
            return;
        };
        if ctx.input(|input| input.pointer.primary_pressed()) && grip.contains(pointer) {
            self.grabbed = Some(pointer.x - edge);
        }
        if let Some(grabbed) = self.grabbed {
            let maximum = (rect.width() / scale - NARROWEST_APP_WIDTH).max(MINIMUM_WIDTH);
            self.width =
                ((rect.right() - pointer.x + grabbed) / scale).clamp(MINIMUM_WIDTH, maximum);
        }
        self.grip = self.grabbed.is_some() || grip.contains(pointer);
    }

    pub fn show(
        &mut self,
        target: &mut Document,
        ctx: &Context,
        layout: &Layout,
        keyboard_interactive: bool,
    ) {
        let Layout {
            bar,
            toolbar,
            content,
            readout,
            panel,
            app_visible,
        } = *layout;
        self.forget_removed(target);
        self.sync(target, ctx);
        let scale = scale(ctx);
        let document = &mut self.document;
        if panel.is_positive() {
            let focused = document.focused_node().is_some();
            ctx.scaled(scale, || {
                let keys = match keyboard_interactive && focused {
                    true => Keys::All,
                    false => Keys::Ignored,
                };
                document.show_content(ctx, panel.scaled(scale.recip()), true, keys);
            });
        }
        self.show_bar(ctx, bar);
        if self.state.reset_performance.take() {
            target.reset_performance();
            ctx.request_repaint();
        }
        ctx.set_touch_emulation(self.state.touch_emulation.get());
        crate::mouse_simulation::simulate(ctx, self.state.mouse_simulation.get());
        target.set_rubber_banding(self.state.rubber_banding.get());
        ctx.set_accessibility_active(self.state.accessibility.get());
        target.track_changes(self.state.flash_changes.get());
        target.track_damage(self.state.flash_damage.get());
        ctx.set_simulated_pixels_per_point(self.state.simulated_pixels_per_point.get());
        if let Some(theme) = self.state.requested_theme.take() {
            target.set_theme(theme);
            ctx.request_repaint();
        }
        if app_visible {
            self.resize(target, ctx, content);
            self.pick(target, ctx, content);
        }
        self.release_focus(ctx);
        if app_visible {
            self.read(target, ctx, content, keyboard_interactive);
            self.paint(target, ctx, content, panel);
            let covering = self.reader.painting();
            if covering {
                self.cover(ctx, content, readout, Layer::Below(target.screen_scale()));
            }
            let screen = target
                .shown_screen()
                .map_or(content, |placement| placement.shown().intersect(content));
            ctx.apply_filter(self.state.filter(screen));
            if covering {
                self.cover(ctx, content, readout, Layer::Above);
            }
            self.paint_handles(target, ctx, content);
        }
        self.show_toolbar(ctx, toolbar, content);
        if target.flashing() {
            ctx.request_repaint();
        }
        if self.state.revision.get() != self.seen {
            self.seen = self.state.revision.get();
            ctx.request_repaint();
        }
    }

    fn screen_simulation(&self) -> Option<ScreenSimulation> {
        let simulation = self.state.screen_simulation.get()?;
        Some(match (self.resizing, simulation.zoom) {
            (Some(resizing), None) => ScreenSimulation {
                zoom: Some(resizing.scale),
                ..simulation
            },
            _ => simulation,
        })
    }

    fn resize(&mut self, target: &Document, ctx: &Context, content: Rect) {
        let Some(simulation) = self.state.screen_simulation.get() else {
            self.resizing = None;
            self.handle = None;
            return;
        };
        if ctx.input(|input| input.pointer.primary_released()) && self.resizing.take().is_some() {
            self.state.touch();
        }
        let pointer = ctx.input(|input| input.pointer.interact_pos());
        if let Some(resizing) = self.resizing {
            if let Some(pointer) = pointer {
                let moved = (pointer - resizing.origin) / resizing.scale;
                let size = resizing.handle.resized(resizing.size, moved);
                self.state
                    .update_screen(|simulation| simulation.resized(size));
            }
            self.handle = Some(resizing.handle);
            return;
        }
        let placement = target.shown_screen();
        self.handle = placement
            .zip(pointer)
            .filter(|_| !self.state.picking.get() && !self.toolbar.open())
            .and_then(|(placement, pointer)| handle_at(placement.shown(), content, pointer));
        if let (Some(handle), Some(placement), Some(origin)) = (self.handle, placement, pointer)
            && ctx.input(|input| input.pointer.primary_pressed())
        {
            self.resizing = Some(Resize {
                handle,
                origin,
                size: simulation.size,
                scale: placement.scale,
            });
        }
    }

    fn paint_handles(&self, target: &Document, ctx: &Context, content: Rect) {
        let placement = target
            .shown_screen()
            .filter(|_| self.state.screen_simulation.get().is_some());
        let painted = ctx.measure_paint(|| {
            let Some(placement) = placement else {
                return;
            };
            let painter = ctx.painter().with_clip_rect(content);
            let shown = placement.shown();
            for handle in [Handle::Right, Handle::Bottom, Handle::Corner] {
                let color = match self.handle == Some(handle) {
                    true => Theme::DARK.accent,
                    false => Theme::DARK.text_muted,
                };
                for grip in handle_grips(shown, handle) {
                    painter.rect_filled(grip, HANDLE_THICKNESS / 2.0, color);
                }
            }
        });
        if let Some(handle) = self.handle {
            ctx.set_cursor_icon(handle.cursor());
        }
        ctx.report_damage(painted.union(self.handle_bounds.replace(painted)));
    }

    fn show_toolbar(&mut self, ctx: &Context, toolbar: Rect, content: Rect) {
        if std::mem::replace(&mut self.toolbar_area, toolbar) != toolbar {
            ctx.report_damage(toolbar);
        }
        if !toolbar.is_positive() {
            return;
        }
        let scale = scale(ctx);
        let focused = self.toolbar.document.focused_node().is_some();
        let responsive::Toolbar {
            document,
            set_screen,
            ..
        } = &mut self.toolbar;
        if let Some(simulation) = self.state.screen_simulation.get() {
            with_reactive_scope(document, || set_screen.set(simulation));
        }
        let keys = match focused {
            true => Keys::All,
            false => Keys::Ignored,
        };
        let area = toolbar.union(content).scaled(scale.recip());
        ctx.scaled(scale, || document.show_content(ctx, area, true, keys));
    }

    fn show_bar(&mut self, ctx: &Context, bar: Rect) {
        if !bar.is_positive() {
            return;
        }
        let scale = scale(ctx);
        let selected = usize::from(!self.state.app_shown.get());
        let panel::Bar {
            document,
            set_selected,
        } = &mut self.bar;
        with_reactive_scope(document, || set_selected.set(selected));
        ctx.scaled(scale, || {
            document.show_content(ctx, bar.scaled(scale.recip()), true, Keys::Ignored);
        });
    }

    fn read(&mut self, target: &Document, ctx: &Context, content: Rect, panel_has_focus: bool) {
        self.reader.configure(self.state.screen_reader.get());
        let commands = self.state.take_commands();
        self.reader
            .show(target, ctx, content, commands, !panel_has_focus);
        if self.state.screen_reader.get() && !self.parked && self.document.focused_node().is_some()
        {
            self.blur();
        }
    }

    fn cover(&mut self, ctx: &Context, content: Rect, readout: Rect, layer: Layer) {
        let scale = scale(ctx);
        let local = scale.recip();
        let Self { reader, .. } = self;
        ctx.scaled(scale, || match layer {
            Layer::Below(screen) => {
                let painter = ctx.painter().with_clip_rect(content.scaled(local));
                reader.paint_focus(&painter, local * screen);
            }
            Layer::Above => {
                let bar = readout.scaled(local);
                let painter = ctx
                    .painter()
                    .with_clip_rect(content.union(readout).scaled(local));
                ctx.report_damage(reader.paint_readout(&painter, local, bar));
            }
        });
    }

    fn forget_removed(&mut self, target: &Document) {
        for cell in [&self.state.hovered, &self.state.selected] {
            if cell.get().is_some_and(|id| !target.contains(id)) {
                cell.set(None);
            }
        }
    }

    fn sync(&mut self, target: &Document, ctx: &Context) {
        let entries = match self.state.tab.get() {
            InspectorTab::Beui => tree::collect(target, &self.state),
            InspectorTab::Components => tree::collect_components(target, &self.state),
            InspectorTab::AccessKit => tree::collect_accesskit(target, &self.state),
            InspectorTab::Performance | InspectorTab::Simulation => Vec::new(),
        };
        let selection = self.selection_path(target);
        let summary = self.summary(target, ctx, &entries, selection.last().copied());
        let performance = panel::PerformanceSummary::from(target.performance());
        let renderer = ctx
            .renderer_info()
            .map(|info| info.rows)
            .unwrap_or_default();
        let Self {
            document,
            set_keys,
            set_entries,
            set_summary,
            set_performance,
            set_renderer,
            set_selection,
            ..
        } = self;
        with_reactive_scope(document, || {
            set_keys.set(entries.iter().map(|entry| entry.key).collect());
            set_entries.set(
                entries
                    .iter()
                    .map(|entry| (entry.key, entry.clone()))
                    .collect(),
            );
            set_summary.set(summary);
            set_performance.set(performance);
            set_renderer.set(renderer);
            set_selection.set(selection);
        });
        self.entries = entries;
    }

    fn summary(
        &self,
        target: &Document,
        ctx: &Context,
        entries: &[Entry],
        selected_key: Option<Key>,
    ) -> Summary {
        let selected = self.state.selected.get();
        let selection = selected.map_or_else(nothing_selected, |id| {
            entries
                .iter()
                .find(|entry| Some(entry.key) == selected_key)
                .or_else(|| entries.iter().find(|entry| entry.key.node() == id))
                .map_or_else(|| tree::label(target, id), entry_label)
        });
        Summary {
            total: match self.state.tab.get() {
                InspectorTab::Beui => target.root().map_or(0, |root| tree::count(target, root)),
                InspectorTab::Components => tree::component_count(target),
                InspectorTab::AccessKit => tree::accesskit_count(target),
                InspectorTab::Performance | InspectorTab::Simulation => 0,
            },
            native_pixel_ratio: native_pixel_ratio_label(ctx.native_pixels_per_point()),
            picking: self.state.picking.get(),
            responsive: self.state.screen_simulation.get().is_some(),
            selection,
            bounds: selected
                .and_then(|id| target.node_rect(id))
                .map(bounds_label)
                .unwrap_or_default(),
        }
    }

    pub fn toggle_focus(&mut self) {
        if self.document.focused_node().is_some() {
            self.blur();
            return;
        }
        let Some(target) = self.focus_entry() else {
            return;
        };
        self.parked = true;
        let Self { document, .. } = self;
        with_reactive_scope(document, || {
            with_document(|document| document.focus_focusable(target))
        });
    }

    fn blur(&mut self) {
        self.parked = false;
        let Self { document, .. } = self;
        with_reactive_scope(document, || {
            with_document(|document| document.update_focus(None))
        });
    }

    fn focus_entry(&self) -> Option<NodeId> {
        let root = self.document.root()?;
        let order = self.document.focusables_within(root);
        let rows = self
            .tree_node()
            .map(|tree| {
                let rows = beui_components_styled::tree_rows(&self.document, tree);
                self.document.focusables_within(rows)
            })
            .unwrap_or_default();
        order
            .iter()
            .find(|id| rows.contains(id))
            .or_else(|| order.first())
            .copied()
    }

    fn tree_node(&self) -> Option<NodeId> {
        self.tree
            .try_get()
            .filter(|tree| self.document.contains(*tree))
    }

    fn release_focus(&mut self, ctx: &Context) {
        if self.state.picking.get() || self.document.focused_node().is_none() {
            return;
        }
        if ctx.input(|input| input.events.iter().any(cancelled)) {
            self.blur();
        }
    }

    fn pick(&mut self, target: &Document, ctx: &Context, content: Rect) {
        if !self.state.picking.get() {
            return;
        }
        if ctx.input(|input| input.events.iter().any(cancelled)) {
            self.state.picking.set(false);
            self.state.touch();
            return;
        }

        self.state.hovered.set(None);
        let pointer = ctx.input(|input| input.pointer.interact_pos());
        let Some(pointer) = pointer.filter(|pointer| content.contains(*pointer)) else {
            return;
        };
        ctx.set_cursor_icon(CursorIcon::Crosshair);
        let screen = target.screen_scale();
        let pointer = pos2(pointer.x / screen, pointer.y / screen);
        let Some(id) = overlay::hit(target, pointer) else {
            return;
        };
        self.state.hovered.set(Some(id));
        if ctx.input(|input| input.pointer.primary_pressed()) {
            self.state.picking.set(false);
            self.state.hovered.set(None);
            self.state.app_shown.set(false);
            self.state.select(id);
        }
    }

    fn selection_path(&self, target: &Document) -> Vec<Key> {
        let Some(id) = self.state.selected.get() else {
            return Vec::new();
        };
        match self.state.tab.get() {
            InspectorTab::Beui => tree::path(target, id),
            InspectorTab::Components => {
                let mut path = tree::component_path(target, id);
                let chosen = self.state.selected_component.get();
                if let Some(end) = path.iter().position(|key| Some(*key) == chosen) {
                    path.truncate(end + 1);
                }
                path
            }
            InspectorTab::AccessKit => tree::accesskit_path(target, id),
            InspectorTab::Performance | InspectorTab::Simulation => Vec::new(),
        }
    }

    fn paint(&self, target: &Document, ctx: &Context, content: Rect, panel: Rect) {
        let scale = scale(ctx);
        let local = scale.recip();
        let screen = local * target.screen_scale();
        ctx.scaled(scale, || {
            let painted = ctx.measure_paint(|| {
                let painter = ctx.painter().with_clip_rect(content.scaled(local));
                flashes(&painter, target, screen);
                let hovered = self.state.hovered.get();
                let selected = self.state.selected.get();
                if let Some(id) = selected.filter(|id| Some(*id) != hovered) {
                    overlay::highlight(&painter, target, id, false, screen);
                }
                if let Some(id) = hovered {
                    overlay::highlight(&painter, target, id, true, screen);
                }
                if self.grip {
                    let panel = panel.scaled(local);
                    let grip = Rect::from_min_max(
                        panel.min,
                        pos2(panel.left() + GRIP_PAINT_WIDTH, panel.bottom()),
                    );
                    ctx.painter().rect_filled(grip, 0.0, Theme::DARK.accent);
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
            });
            ctx.report_damage(painted.union(self.overlay_bounds.replace(painted)));
        });
    }
}

fn flashes(painter: &Painter, target: &Document, scale: f32) {
    let now = Instant::now();
    for (id, at) in target.change_flashes() {
        let Some(rect) = target.node_rect(id) else {
            continue;
        };
        overlay::flash(
            painter,
            rect.scaled(scale),
            flash::CHANGE,
            flash::remaining(now, at),
        );
    }
    for (rect, at) in target.damage_flashes() {
        overlay::flash_outline(
            painter,
            rect.scaled(scale),
            flash::REPAINT,
            flash::remaining(now, at),
        );
    }
}

fn handle_at(shown: Rect, content: Rect, pointer: Pos2) -> Option<Handle> {
    if !content.contains(pointer) {
        return None;
    }
    let right = (shown.right()..=shown.right() + HANDLE_REACH).contains(&pointer.x);
    let below = (shown.bottom()..=shown.bottom() + HANDLE_REACH).contains(&pointer.y);
    let beside = (shown.top()..shown.bottom()).contains(&pointer.y);
    let above = (shown.left()..shown.right()).contains(&pointer.x);
    match (right, below) {
        (true, true) => Some(Handle::Corner),
        (true, false) if beside => Some(Handle::Right),
        (false, true) if above => Some(Handle::Bottom),
        _ => None,
    }
}

fn handle_grips(shown: Rect, handle: Handle) -> Vec<Rect> {
    let gap = (HANDLE_REACH - HANDLE_THICKNESS) / 2.0;
    let right = shown.right() + gap;
    let bottom = shown.bottom() + gap;
    let length = HANDLE_LENGTH.min(shown.height()).min(shown.width());
    match handle {
        Handle::Right => vec![Rect::from_min_size(
            pos2(right, shown.center().y - length / 2.0),
            vec2(HANDLE_THICKNESS, length),
        )],
        Handle::Bottom => vec![Rect::from_min_size(
            pos2(shown.center().x - length / 2.0, bottom),
            vec2(length, HANDLE_THICKNESS),
        )],
        Handle::Corner => vec![
            Rect::from_min_size(
                pos2(right, bottom - HANDLE_CORNER + HANDLE_THICKNESS),
                vec2(HANDLE_THICKNESS, HANDLE_CORNER),
            ),
            Rect::from_min_size(
                pos2(right - HANDLE_CORNER + HANDLE_THICKNESS, bottom),
                vec2(HANDLE_CORNER, HANDLE_THICKNESS),
            ),
        ],
    }
}

fn split(rect: Rect, width: f32) -> (Rect, Rect) {
    let edge = rect.right() - width;
    (
        Rect::from_min_max(rect.min, pos2(edge, rect.bottom())),
        Rect::from_min_max(pos2(edge, rect.top()), rect.max),
    )
}

fn trim_top(rect: Rect, height: f32) -> (Rect, Rect) {
    let height = height.clamp(0.0, rect.height().max(0.0));
    let edge = rect.top() + height;
    (
        Rect::from_min_max(rect.min, pos2(rect.right(), edge)),
        Rect::from_min_max(pos2(rect.left(), edge), rect.max),
    )
}

pub fn scale(ctx: &Context) -> f32 {
    ctx.native_pixels_per_point() / ctx.pixels_per_point()
}

fn entry_label(entry: &Entry) -> String {
    if entry.detail.is_empty() {
        entry.kind.clone()
    } else {
        format!("{} {}", entry.kind, entry.detail)
    }
}

fn cancelled(event: &Event) -> bool {
    matches!(
        event,
        Event::Key {
            key: InputKey::Escape,
            pressed: true,
            ..
        }
    )
}

fn nothing_selected() -> String {
    "nothing selected".to_owned()
}

fn native_pixel_ratio_label(pixels_per_point: f32) -> String {
    format!(
        "Native pixel ratio: {}x",
        (pixels_per_point * 100.0).round() / 100.0
    )
}

fn bounds_label(rect: Rect) -> String {
    format!(
        "{}, {}  {} x {}",
        rect.left().round(),
        rect.top().round(),
        rect.width().round(),
        rect.height().round()
    )
}

#[derive(Default)]
pub struct InspectorTools {
    pub inspector: Option<Box<Inspector>>,
}

pub fn install(document: &mut Document) {
    document.set_tools(Box::new(InspectorTools::default()));
}

impl Tools for InspectorTools {
    fn show(&mut self, document: &mut Document, ctx: &Context, rect: Rect) {
        let theme = document.theme();
        if chord_pressed(ctx, InputKey::I) {
            self.inspector = match self.inspector {
                Some(_) => None,
                None => Some(Box::new(Inspector::new(ctx, theme))),
            };
        }
        if chord_pressed(ctx, InputKey::C) {
            self.inspector
                .get_or_insert_with(|| Box::new(Inspector::new(ctx, theme)))
                .toggle_picking();
        }
        if chord_pressed(ctx, InputKey::M) {
            self.inspector
                .get_or_insert_with(|| Box::new(Inspector::new(ctx, theme)))
                .toggle_responsive();
        }
        if chord_pressed(ctx, InputKey::F)
            && let Some(inspector) = self.inspector.as_mut()
        {
            inspector.toggle_focus();
        }

        if self.inspector.is_none() {
            document.track_changes(false);
            document.track_damage(false);
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
            document.show_screen(ctx, layout.content, !intercepted, keys);
        } else {
            document.hide();
        }
        if document.take_inspector_request() && self.inspector.is_none() {
            self.inspector = Some(Box::new(Inspector::new(ctx, document.theme())));
            ctx.request_repaint();
        }

        if let Some(mut inspector) = self.inspector.take() {
            inspector.show(document, ctx, &layout, inspector_has_focus);
            if !inspector.closed() {
                self.inspector = Some(inspector);
            }
        }
        ctx.show_mouse_simulation(viewport);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn chord_pressed(ctx: &Context, chord: InputKey) -> bool {
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
