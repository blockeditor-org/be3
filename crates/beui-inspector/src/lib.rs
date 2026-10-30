extern crate beui_view as beui;

pub mod mouse_simulation;
pub mod overlay;
pub mod panel;
pub mod responsive;
pub mod screen_reader;
pub mod tree;

use stable_hash::HashMap;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_core::base::embed::EmbedSlot;
use beui_core::context::Context;
use beui_core::filter::{ColorVision, Filter};
use beui_core::flash;
use beui_core::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use beui_core::input::{
    BackGesture, CursorIcon, Event, InputState, Key as InputKey, PointerButton, PointerPhase,
};
use beui_core::painter::Painter;

use crate::screen_reader::{Command, ScreenReader};
use beui_components_styled::Theme;
use beui_core::current::{with_document, with_reactive_scope};
use beui_core::document::{Document, Tools, trim_bottom};
use beui_core::layer::LayerPolicy;
use beui_core::node::NodeId;
use beui_core::preferences::{PreferenceLayer, PreferenceSource};
use beui_core::screen_simulation::ScreenSimulation;
use beui_core::window::WindowId;
use beui_dock::unstyled::{DockMode, DockingLayout};
use beui_view::reactive::*;

use panel::Summary;
use tree::{Entry, Key};

const DEFAULT_WIDTH: f32 = 340.0;
const HANDLE_REACH: f32 = beui_core::screen_simulation::MARGIN;
const HANDLE_THICKNESS: f32 = 4.0;
const HANDLE_LENGTH: f32 = 40.0;
const HANDLE_CORNER: f32 = 14.0;

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub enum InspectorTab {
    App,
    #[default]
    Beui,
    Components,
    AccessKit,
    Performance,
    Simulation,
}

impl InspectorTab {
    pub const TREES: [InspectorTab; 3] = [
        InspectorTab::Beui,
        InspectorTab::Components,
        InspectorTab::AccessKit,
    ];

    pub fn title(self) -> &'static str {
        match self {
            InspectorTab::App => "App",
            InspectorTab::Beui => "Beui",
            InspectorTab::Components => "Comp",
            InspectorTab::AccessKit => "A11y",
            InspectorTab::Performance => "Perf",
            InspectorTab::Simulation => "Sim",
        }
    }
}

impl beui_dock::unstyled::DockKey for InspectorTab {
    fn key(&self) -> String {
        format!("{self:?}")
    }
}

pub struct State {
    expansion: RefCell<HashMap<Key, bool>>,
    pub hovered: Cell<Option<NodeId>>,
    pub selected: Cell<Option<NodeId>>,
    selected_component: Cell<Option<Key>>,
    pub picking: Cell<bool>,
    pub touch_emulation: Cell<bool>,
    pub mouse_simulation: Cell<bool>,
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
    back_gestures: RefCell<Vec<BackGesture>>,
    pub developer: Cell<PreferenceLayer>,
    requested_developer: Cell<Option<PreferenceLayer>>,
    revision: Cell<u64>,
    reset_performance: Cell<bool>,
    closed: Cell<bool>,
    keyboard_toggled: Cell<bool>,
    reveal: Cell<Option<Reveal>>,
    release_keyboard: Cell<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Reveal {
    App,
    Tool,
}

impl State {
    fn new(ctx: &Context) -> Self {
        Self {
            expansion: RefCell::new(HashMap::default()),
            hovered: Cell::new(None),
            selected: Cell::new(None),
            selected_component: Cell::new(None),
            picking: Cell::new(false),
            touch_emulation: Cell::new(ctx.touch_emulation()),
            mouse_simulation: Cell::new(ctx.mouse_simulation()),
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
            back_gestures: RefCell::new(Vec::new()),
            developer: Cell::new(ctx.preference_layer(PreferenceSource::Developer)),
            requested_developer: Cell::new(None),
            revision: Cell::new(0),
            reset_performance: Cell::new(false),
            closed: Cell::new(false),
            keyboard_toggled: Cell::new(false),
            reveal: Cell::new(None),
            release_keyboard: Cell::new(false),
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

    fn reset_performance(&self) {
        self.reset_performance.set(true);
        self.touch();
    }

    fn close(&self) {
        self.closed.set(true);
        self.touch();
    }

    fn toggle_keyboard(&self) {
        self.keyboard_toggled.set(true);
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

    fn simulate_back(&self, gesture: BackGesture) {
        self.back_gestures.borrow_mut().push(gesture);
        self.touch();
    }

    fn take_back_gestures(&self) -> Vec<BackGesture> {
        std::mem::take(&mut self.back_gestures.borrow_mut())
    }

    fn override_preferences(&self, layer: PreferenceLayer) {
        self.developer.set(layer);
        self.requested_developer.set(Some(layer));
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
        if matches!(key, Key::Component(..) | Key::Internals(..) | Key::Text(..)) {
            self.selected_component.set(Some(key));
        }
    }

    fn toggle_picking(&self) {
        if self.picking.get() {
            self.stop_picking();
            return;
        }
        self.picking.set(true);
        self.reveal.set(Some(Reveal::App));
        self.touch();
    }

    fn stop_picking(&self) {
        self.picking.set(false);
        self.hovered.set(None);
        self.touch();
    }

    fn release_keyboard(&self) {
        self.release_keyboard.set(true);
        self.touch();
    }

    fn touch(&self) {
        self.revision.set(self.revision.get() + 1);
    }
}

pub struct Layout {
    pub toolbar: Rect,
    pub content: Rect,
    pub readout: Rect,
    pub app_visible: bool,
}

impl Layout {
    pub fn app(rect: Rect) -> Self {
        Self {
            toolbar: Rect::NOTHING,
            content: rect,
            readout: Rect::NOTHING,
            app_visible: true,
        }
    }

    fn hidden() -> Self {
        Self {
            app_visible: false,
            ..Self::app(Rect::NOTHING)
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
    reader: ScreenReader,
    trees: [panel::TreeView; 3],
    tools: DockingLayout<InspectorTab>,
    synced_shown: [bool; 3],
    summary: Signal<Summary>,
    performance: Signal<panel::PerformanceSummary>,
    renderer: Signal<panel::RendererRows>,
    app: EmbedSlot,
    width: Signal<f32>,
    last_tool: InspectorTab,
    toolbar: responsive::Toolbar,
    resizing: Option<Resize>,
    handle: Option<Handle>,
    handle_bounds: Cell<Rect>,
    toolbar_area: Rect,
    held: bool,
    letting_go: bool,
    parked: bool,
    seen: u64,
    overlay_bounds: Cell<Rect>,
    embeds_app: bool,
    over: Option<(WindowId, Rect)>,
    _panel_scope: ::reactive::Scope,
    state: OwnedStored<State>,
}

impl Inspector {
    pub fn new(ctx: &Context, rect: Rect) -> Self {
        let state = OwnedStored::new(State::new(ctx));
        let embeds_app = ctx.shows_main();
        let panel = panel::build(state.handle(), rect.width() / scale(ctx), embeds_app);
        let toolbar = responsive::build(state.handle());
        Self {
            toolbar,
            resizing: None,
            handle: None,
            handle_bounds: Cell::new(Rect::NOTHING),
            toolbar_area: Rect::NOTHING,
            document: panel.document,
            _panel_scope: panel.scope,
            entries: Vec::new(),
            trees: panel.trees,
            tools: panel.tools,
            synced_shown: [false; 3],
            state,
            reader: ScreenReader::default(),
            summary: panel.summary,
            performance: panel.performance,
            renderer: panel.renderer,
            app: panel.app,
            width: panel.width,
            last_tool: InspectorTab::default(),
            held: false,
            letting_go: false,
            parked: false,
            seen: 0,
            overlay_bounds: Cell::new(Rect::NOTHING),
            embeds_app,
            over: None,
        }
    }

    pub fn state(&self) -> Rc<State> {
        self.state.value()
    }

    pub fn row_node(&self, index: usize) -> NodeId {
        self.find(&self.entries[index].key.test_id())
    }

    pub fn find(&self, test_id: &str) -> NodeId {
        self.document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("the inspector has no node with test id {test_id:?}"))
    }

    pub fn tools_stacked(&self) -> bool {
        self.tools.mode().get_untracked() == DockMode::Stacked
    }

    pub fn tab_node(&self, tab: InspectorTab) -> NodeId {
        if self.tools_stacked() {
            let id = self.tools.tab_of(&tab).expect("every tool is a tab");
            return self.find(&format!("dock.switcher.tab.{}", id.value()));
        }
        let root = self.document.root().expect("the inspector has a root");
        placed_text(&self.document, root, tab.title())
            .unwrap_or_else(|| panic!("the inspector shows no {} tab", tab.title()))
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

    pub fn app_visible(&self) -> bool {
        self.app.placement().is_some()
    }

    pub fn app_rect(&self) -> Option<Rect> {
        self.app.placement().map(|placement| placement.rect)
    }

    pub fn reader(&self) -> &ScreenReader {
        &self.reader
    }

    pub fn selected_row(&self, target: &Document) -> Option<String> {
        let tab = InspectorTab::TREES[self.current_tree()?];
        let key = self.selection_path(target, tab).last().copied()?;
        let entry = self.entries.iter().find(|entry| entry.key == key)?;
        Some(entry.kind.clone())
    }

    pub fn focused_row(&self) -> Option<Key> {
        self.tree_api()?.focused()
    }

    pub fn highlighted(&self) -> Option<NodeId> {
        self.state().hovered.get().or_else(|| {
            let tree = self.tree_api()?;
            self.document
                .watch_focus_visible()
                .get_untracked()
                .then(|| tree.focused())
                .flatten()
                .map(Key::node)
        })
    }

    pub fn closed(&self) -> bool {
        self.state().closed.get()
    }

    pub fn readout_height(&self, ctx: &Context) -> f32 {
        match self.state().screen_reader.get() {
            true => ScreenReader::height() * scale(ctx),
            false => 0.0,
        }
    }

    pub fn show_panel(
        &mut self,
        target: &Document,
        ctx: &Context,
        rect: Rect,
        keyboard_interactive: bool,
    ) -> Layout {
        ctx.set_screen_simulation(self.screen_simulation());
        let scale = scale(ctx);
        let width = rect.width() / scale;
        if self.width.get_untracked() != width {
            self.width.set(width);
        }
        if let Some(tab) = self.tools.focused().filter(|tab| *tab != InspectorTab::App) {
            self.last_tool = tab;
        }
        let revealed = self.state().reveal.get().map(|reveal| match reveal {
            Reveal::App => InspectorTab::App,
            Reveal::Tool => self.last_tool,
        });
        if let Some(tab) = revealed.filter(|tab| self.tools.tab_of(tab).is_some()) {
            self.tools.show(&tab);
            self.state().reveal.set(None);
        }
        self.forget_removed(target);
        self.sync(target, ctx);
        let document = &mut self.document;
        let keys = keyboard_interactive && document.focused_node().is_some();
        ctx.scaled(scale, || {
            ctx.with_input(
                |input| keyed(input, keys),
                || document.show_content(ctx, rect.scaled(scale.recip())),
            );
        });
        if self.tools_shown() != self.synced_shown {
            ctx.request_repaint();
        }
        let Some(content) = self.app_rect().map(|app| app.scaled(scale)) else {
            return Layout::hidden();
        };
        if self.state().screen_simulation.get().is_none() {
            return Layout::app(content);
        }
        let (toolbar, content) = trim_top(content, responsive::HEIGHT * scale);
        Layout {
            toolbar,
            ..Layout::app(content)
        }
    }

    fn show_over(&mut self, target: &Document, ctx: &Context, keyboard_interactive: bool) {
        let window = beui_core::software_keyboard::typing_window(target, ctx);
        self.over = target.window_rect(window).map(|shown| {
            let width = (DEFAULT_WIDTH * scale(ctx)).min(shown.width());
            let left = shown.right() - width;
            (
                window,
                Rect::from_min_max(pos2(left, shown.top()), shown.max),
            )
        });
        let Some((window, rect)) = self.over else {
            return;
        };
        ctx.with_input(
            |input| input.seen_from(window),
            || {
                ctx.draw_into(window, || {
                    self.show_panel(target, ctx, rect, keyboard_interactive);
                });
            },
        );
    }

    pub fn hold(&mut self, ctx: &Context) {
        if std::mem::take(&mut self.letting_go) {
            self.held = false;
        }
        for (down, at) in primary(ctx) {
            match down {
                true => self.held = self.covers(ctx, at),
                false => self.letting_go = self.held,
            }
        }
    }

    pub fn covers(&self, ctx: &Context, pos: Pos2) -> bool {
        if let Some((window, rect)) = self.over {
            if ctx.input(|input| input.pointer.window) != window {
                return false;
            }
            if rect.contains(pos) {
                return true;
            }
        }
        let scale = scale(ctx);
        let pos = pos2(pos.x / scale, pos.y / scale);
        let layers = self.document.layers();
        layers.iter().skip(1).any(|layer| {
            layer.window.is_main()
                && layer.policy != LayerPolicy::Inert
                && self
                    .document
                    .layer_covers(layer)
                    .iter()
                    .any(|cover| cover.contains(pos))
        })
    }

    pub fn has_focus(&self) -> bool {
        self.document.focused_node().is_some() || self.toolbar.document.focused_node().is_some()
    }

    pub fn intercepts(&self) -> bool {
        self.state().picking.get()
            || self.held
            || self.resizing.is_some()
            || self.toolbar.open()
            || self.state().screen_reader.get()
    }

    pub fn takes_keys(&self) -> bool {
        self.state().picking.get() || self.resizing.is_some()
    }

    pub fn claims(&self, event: &Event) -> bool {
        self.state().screen_reader.get() && crate::screen_reader::claims(event)
    }

    pub fn toggle_picking(&self) {
        self.state().toggle_picking();
    }

    pub fn toggle_responsive(&self) {
        self.state().toggle_responsive();
    }

    pub fn show(
        &mut self,
        target: &mut Document,
        ctx: &Context,
        layout: &Layout,
        keyboard_interactive: bool,
    ) {
        let Layout {
            toolbar,
            content,
            readout,
            app_visible,
        } = *layout;
        if self.state().release_keyboard.take() {
            self.blur();
        }
        if self.state().reset_performance.take() {
            target.reset_performance();
            ctx.request_repaint();
        }
        if self.state().keyboard_toggled.take() {
            ctx.toggle_keyboard();
        }
        ctx.set_touch_emulation(self.state().touch_emulation.get());
        crate::mouse_simulation::simulate(ctx, self.state().mouse_simulation.get());
        if let Some(layer) = self.state().requested_developer.take() {
            ctx.set_preference_layer(PreferenceSource::Developer, layer);
        }
        ctx.set_accessibility_active(self.state().accessibility.get());
        target.track_changes(self.state().flash_changes.get());
        target.track_damage(self.state().flash_damage.get());
        ctx.set_simulated_pixels_per_point(self.state().simulated_pixels_per_point.get());
        if app_visible {
            self.resize(target, ctx, content);
            self.pick(target, ctx, content);
        }
        self.release_focus(ctx);
        if app_visible {
            self.read(target, ctx, content, keyboard_interactive);
            self.paint(target, ctx, content);
            let covering = self.reader.painting();
            if covering {
                self.cover(ctx, content, readout, Layer::Below(target.screen_scale()));
            }
            let screen = target
                .shown_screen()
                .map_or(content, |placement| placement.shown().intersect(content));
            ctx.apply_filter(self.state().filter(screen));
            if covering {
                self.cover(ctx, content, readout, Layer::Above);
            }
            self.paint_handles(target, ctx, content);
        }
        self.show_toolbar(ctx, toolbar, content);
        if target.flashing() {
            ctx.request_repaint();
        }
        if self.state().revision.get() != self.seen {
            self.seen = self.state().revision.get();
            ctx.request_repaint();
        }
    }

    fn screen_simulation(&self) -> Option<ScreenSimulation> {
        let simulation = self.state().screen_simulation.get()?;
        Some(match (self.resizing, simulation.zoom) {
            (Some(resizing), None) => ScreenSimulation {
                zoom: Some(resizing.scale),
                ..simulation
            },
            _ => simulation,
        })
    }

    fn resize(&mut self, target: &Document, ctx: &Context, content: Rect) {
        let Some(simulation) = self.state().screen_simulation.get() else {
            self.resizing = None;
            self.handle = None;
            return;
        };
        let placement = target.shown_screen();
        let reachable = !self.state().picking.get() && !self.toolbar.open();
        let handle_under = |pos: Pos2| {
            placement
                .filter(|_| reachable)
                .and_then(|placement| handle_at(placement.shown(), content, pos))
        };
        for (down, at) in primary(ctx) {
            match down {
                true if self.resizing.is_none() => {
                    if let (Some(handle), Some(placement)) = (handle_under(at), placement) {
                        self.resizing = Some(Resize {
                            handle,
                            origin: at,
                            size: simulation.size,
                            scale: placement.scale,
                        });
                    }
                }
                true => {}
                false => {
                    if let Some(resizing) = self.resizing.take() {
                        let moved = (at - resizing.origin) / resizing.scale;
                        let size = resizing.handle.resized(resizing.size, moved);
                        self.state()
                            .update_screen(|simulation| simulation.resized(size));
                        self.state().touch();
                    }
                }
            }
        }
        let pointer = ctx.input(|input| input.pointer.interact_pos());
        if let Some(resizing) = self.resizing {
            if let Some(pointer) = pointer {
                let moved = (pointer - resizing.origin) / resizing.scale;
                let size = resizing.handle.resized(resizing.size, moved);
                self.state()
                    .update_screen(|simulation| simulation.resized(size));
            }
            self.handle = Some(resizing.handle);
            return;
        }
        self.handle = pointer.and_then(handle_under);
    }

    fn paint_handles(&self, target: &Document, ctx: &Context, content: Rect) {
        let placement = target
            .shown_screen()
            .filter(|_| self.state().screen_simulation.get().is_some());
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
            self.toolbar.document.hide();
            return;
        }
        let scale = scale(ctx);
        let focused = self.toolbar.document.focused_node().is_some();
        let responsive::Toolbar {
            document, screen, ..
        } = &mut self.toolbar;
        if let Some(simulation) = self.state.value().screen_simulation.get() {
            screen.set(simulation);
        }
        let area = toolbar.union(content).scaled(scale.recip());
        ctx.scaled(scale, || {
            ctx.with_input(
                |input| keyed(input, focused),
                || document.show_content(ctx, area),
            );
        });
    }

    fn read(&mut self, target: &Document, ctx: &Context, content: Rect, panel_has_focus: bool) {
        self.reader.configure(self.state().screen_reader.get());
        let commands = self.state().take_commands();
        self.reader
            .show(target, ctx, content, commands, !panel_has_focus);
        if self.state().screen_reader.get()
            && !self.parked
            && self.document.focused_node().is_some()
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
        let state = self.state();
        for cell in [&state.hovered, &state.selected] {
            if cell.get().is_some_and(|id| !target.contains(id)) {
                cell.set(None);
            }
        }
    }

    fn tools_shown(&self) -> [bool; 3] {
        InspectorTab::TREES.map(|tab| self.tab_shown(tab))
    }

    fn tab_shown(&self, tab: InspectorTab) -> bool {
        let tools = self.tools;
        let stacked = tools.mode().get_untracked() == DockMode::Stacked;
        tools
            .state()
            .with_untracked(|state| match state.is_empty() {
                true => tab == InspectorTab::default(),
                false => tools
                    .tab_of(&tab)
                    .is_some_and(|id| state.is_shown(id, stacked)),
            })
    }

    fn current_tree(&self) -> Option<usize> {
        let shown = self.tools_shown();
        let focused = self
            .tools
            .focused()
            .and_then(|tab| InspectorTab::TREES.iter().position(|tree| *tree == tab))
            .filter(|index| shown[*index]);
        focused.or_else(|| shown.iter().position(|shown| *shown))
    }

    fn sync(&mut self, target: &Document, ctx: &Context) {
        let shown = self.tools_shown();
        let current = self.current_tree();
        let selected_key = |selection: &[Key]| selection.last().copied();
        let mut current_entries = Vec::new();
        for (index, tab) in InspectorTab::TREES.into_iter().enumerate() {
            if !shown[index] {
                continue;
            }
            let entries = match tab {
                InspectorTab::Beui => tree::collect(target, &self.state()),
                InspectorTab::Components => tree::collect_components(target, &self.state()),
                _ => tree::collect_accesskit(target, &self.state()),
            };
            let selection = self.selection_path(target, tab);
            let view = self.trees[index];
            view.keys
                .set(entries.iter().map(|entry| entry.key).collect());
            view.entries.set(
                entries
                    .iter()
                    .map(|entry| (entry.key, entry.clone()))
                    .collect(),
            );
            view.selection_text.set(self.selection_text(
                target,
                &entries,
                selected_key(&selection),
            ));
            view.total.set(total(target, tab));
            view.selection.set(selection);
            if current == Some(index) {
                current_entries = entries;
            }
        }
        self.synced_shown = shown;
        let summary = self.summary(target, ctx);
        let performance = panel::PerformanceSummary::from(target.performance());
        let renderer = ctx
            .renderer_info()
            .map(|info| info.rows)
            .unwrap_or_default();
        self.summary.set(summary);
        self.performance.set(performance);
        self.renderer.set(renderer);
        self.entries = current_entries;
    }

    fn selection_text(
        &self,
        target: &Document,
        entries: &[Entry],
        selected_key: Option<Key>,
    ) -> String {
        self.state()
            .selected
            .get()
            .map_or_else(nothing_selected, |id| {
                entries
                    .iter()
                    .find(|entry| Some(entry.key) == selected_key)
                    .or_else(|| entries.iter().find(|entry| entry.key.node() == id))
                    .map_or_else(|| tree::label(target, id), entry_label)
            })
    }

    fn summary(&self, target: &Document, ctx: &Context) -> Summary {
        let selected = self.state().selected.get();
        Summary {
            native_pixel_ratio: native_pixel_ratio_label(ctx.native_pixels_per_point()),
            picking: self.state().picking.get(),
            responsive: self.state().screen_simulation.get().is_some(),
            handles_back: target.handles_back(),
            bounds: selected
                .and_then(|id| target.node_rect(id))
                .map(bounds_label)
                .unwrap_or_default(),
            properties: selected
                .map(|id| tree::properties(target, id))
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
            .tree_api()
            .map(|tree| self.document.focusables_within(tree.node()))
            .unwrap_or_default();
        order
            .iter()
            .find(|id| rows.contains(id))
            .or_else(|| order.first())
            .copied()
    }

    fn tree_api(&self) -> Option<beui_components_styled::TreeApi<Key>> {
        self.trees[self.current_tree()?]
            .tree
            .try_get()
            .filter(|tree| self.document.contains(tree.node()))
    }

    fn release_focus(&mut self, ctx: &Context) {
        if self.state().picking.get() || self.document.focused_node().is_none() {
            return;
        }
        if ctx.input(|input| input.events.iter().any(cancelled)) {
            self.blur();
        }
    }

    fn pick(&mut self, target: &Document, ctx: &Context, content: Rect) {
        if !self.state().picking.get() {
            return;
        }
        if ctx.input(|input| input.events.iter().any(cancelled)) {
            self.state().stop_picking();
            return;
        }

        self.state().hovered.set(None);
        let screen = target.screen_scale();
        let under = |pos: Pos2| {
            let pos = Some(pos).filter(|pos| content.contains(*pos))?;
            overlay::hit(target, pos2(pos.x / screen, pos.y / screen))
        };
        let pressed = primary(ctx).into_iter().find(|(down, _)| *down);
        if let Some(id) = pressed.and_then(|(_, at)| under(at)) {
            self.state().stop_picking();
            self.state().reveal.set(Some(Reveal::Tool));
            self.state().select(id);
            return;
        }
        let pointer = ctx.input(|input| input.pointer.interact_pos());
        let Some(pointer) = pointer.filter(|pointer| content.contains(*pointer)) else {
            return;
        };
        ctx.set_cursor_icon(CursorIcon::Crosshair);
        if let Some(id) = under(pointer) {
            self.state().hovered.set(Some(id));
        }
    }

    fn selection_path(&self, target: &Document, tab: InspectorTab) -> Vec<Key> {
        let Some(id) = self.state().selected.get() else {
            return Vec::new();
        };
        match tab {
            InspectorTab::Beui => tree::path(target, id),
            InspectorTab::Components => {
                tree::component_path(target, id, self.state().selected_component.get())
            }
            InspectorTab::AccessKit => tree::accesskit_path(target, id),
            InspectorTab::App | InspectorTab::Performance | InspectorTab::Simulation => Vec::new(),
        }
    }

    fn paint(&self, target: &Document, ctx: &Context, content: Rect) {
        let scale = scale(ctx);
        let local = scale.recip();
        let screen = local * target.screen_scale();
        ctx.scaled(scale, || {
            let painted = ctx.measure_paint(|| {
                let painter = ctx.painter().with_clip_rect(content.scaled(local));
                flashes(&painter, target, screen);
                if let Some(id) = self.highlighted() {
                    overlay::highlight(&painter, target, id, screen);
                }
            });
            ctx.report_damage(painted.union(self.overlay_bounds.replace(painted)));
        });
    }
}

fn flashes(painter: &Painter, target: &Document, scale: f32) {
    let now = target.now();
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
    event
        .press()
        .is_some_and(|press| press.key == InputKey::Escape && press.pressed)
}

fn total(target: &Document, tab: InspectorTab) -> usize {
    match tab {
        InspectorTab::Beui => target.root().map_or(0, |root| tree::count(target, root)),
        InspectorTab::Components => tree::component_count(target),
        InspectorTab::AccessKit => tree::accesskit_count(target),
        InspectorTab::App | InspectorTab::Performance | InspectorTab::Simulation => 0,
    }
}

fn placed_text(document: &Document, root: NodeId, text: &str) -> Option<NodeId> {
    if document
        .arena
        .kind_of::<beui_core::base::text::TextNode>(root)
        .is_some_and(|node| document.text(node) == text)
        && document.node_rect(root).is_some()
    {
        return Some(root);
    }
    document
        .children(root)
        .into_iter()
        .find_map(|child| placed_text(document, child, text))
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
    withheld: Withheld,
    over: Option<(WindowId, Rect)>,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Withheld {
    pointer: bool,
    keys: bool,
}

pub fn install(document: &mut Document) {
    document.set_tools(Box::new(InspectorTools::default()));
}

impl Tools for InspectorTools {
    fn show(&mut self, document: &mut Document, ctx: &Context, rect: Rect) {
        let viewport = rect;
        let rect = trim_bottom(rect, ctx.measure_mouse_simulation(viewport)).0;
        let open = || Box::new(Inspector::new(ctx, rect));
        let focused = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.has_focus());
        let forwarded = document.forwards_keys() && !focused;
        let chord_pressed = |chord| !forwarded && chord_pressed(ctx, chord);
        if chord_pressed(InputKey::I) {
            self.inspector = match self.inspector {
                Some(_) => None,
                None => Some(open()),
            };
        }
        if chord_pressed(InputKey::C) && !document.clipboard_keys() {
            self.inspector.get_or_insert_with(open).toggle_picking();
        }
        if chord_pressed(InputKey::M) {
            self.inspector.get_or_insert_with(open).toggle_responsive();
        }
        if chord_pressed(InputKey::F)
            && let Some(inspector) = self.inspector.as_mut()
        {
            inspector.toggle_focus();
        }

        if self.inspector.is_none() {
            document.track_changes(false);
            document.track_damage(false);
        }

        let inspector_has_focus = self
            .inspector
            .as_ref()
            .is_some_and(|inspector| inspector.has_focus());
        let (mut layout, panel) = match &mut self.inspector {
            Some(inspector) if inspector.embeds_app => {
                ctx.set_aside(|| inspector.show_panel(document, ctx, rect, inspector_has_focus))
            }
            Some(_) | None => (Layout::app(rect), Vec::new()),
        };
        let reserved = match &self.inspector {
            Some(inspector) if layout.app_visible => inspector.readout_height(ctx),
            _ => 0.0,
        };
        (layout.content, layout.readout) = trim_bottom(layout.content, reserved);
        if let Some(inspector) = self.inspector.as_mut() {
            inspector.hold(ctx);
        }
        let pointer = ctx.input(|input| input.pointer.interact_pos());
        let intercepted = self.inspector.as_ref().is_some_and(|inspector| {
            inspector.intercepts() || pointer.is_some_and(|pointer| inspector.covers(ctx, pointer))
        });
        let withheld = Withheld {
            pointer: intercepted,
            keys: inspector_has_focus
                || self
                    .inspector
                    .as_ref()
                    .is_some_and(|inspector| inspector.takes_keys()),
        };
        let was = std::mem::replace(&mut self.withheld, withheld);
        if let Some(inspector) = &self.inspector {
            for gesture in inspector.state().take_back_gestures() {
                with_reactive_scope(document, || {
                    with_document(|document| document.back(gesture))
                });
            }
        }
        if layout.app_visible {
            let inspector = self.inspector.as_deref();
            let given = |input: &InputState| {
                let input = match withheld.pointer {
                    true => input.without_pointer(!was.pointer),
                    false => input.clone(),
                };
                let input = match (withheld.keys, was.keys) {
                    (true, false) => input
                        .only(|event| !event.is_key())
                        .with_event(Event::Focus(false)),
                    (true, true) => {
                        input.only(|event| !event.is_key() && *event != Event::Focus(false))
                    }
                    (false, true) => input.with_event(Event::Focus(true)),
                    (false, false) => input,
                };
                input.only(|event| !inspector.is_some_and(|inspector| inspector.claims(event)))
            };
            ctx.with_input(given, || {
                beui_core::software_keyboard::show_with_keyboard(
                    document,
                    ctx,
                    layout.content,
                    |document, content| document.show_screen(ctx, content),
                )
            });
        } else {
            document.hide();
        }
        if let Some(inspector) = self
            .inspector
            .as_mut()
            .filter(|inspector| !inspector.embeds_app)
        {
            inspector.show_over(document, ctx, inspector_has_focus);
        }
        if let Some(mut inspector) = self.inspector.take() {
            let layout = Layout {
                app_visible: layout.app_visible && inspector.embeds_app,
                ..layout
            };
            inspector.show(document, ctx, &layout, inspector_has_focus);
            if !inspector.closed() {
                self.inspector = Some(inspector);
            }
        }
        ctx.put_back(panel);
        let over = self.inspector.as_ref().and_then(|inspector| inspector.over);
        if let Some((window, rect)) = std::mem::replace(&mut self.over, over)
            && over != Some((window, rect))
        {
            ctx.draw_into(window, || ctx.report_damage(rect));
        }
        if document.take_inspector_request() && self.inspector.is_none() {
            self.inspector = Some(open());
            ctx.request_repaint();
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
        input.events.iter().filter_map(Event::press).any(|press| {
            press.pressed && press.key == chord && press.modifiers.ctrl && press.modifiers.shift
        })
    })
}

fn keyed(input: &InputState, keys: bool) -> InputState {
    match keys {
        true => input.clone(),
        false => input.only(|event| !event.is_key()),
    }
}

fn primary(ctx: &Context) -> Vec<(bool, Pos2)> {
    ctx.input(|input| {
        input
            .pointer_events
            .iter()
            .filter_map(|event| match event.phase {
                PointerPhase::Down(PointerButton::Primary) => Some((true, event.pos)),
                PointerPhase::Up(PointerButton::Primary) => Some((false, event.pos)),
                _ => None,
            })
            .collect()
    })
}
