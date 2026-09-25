mod overlay;
mod panel;
mod tree;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::context::Context;
use crate::filter::{ColorVision, Filter};
use crate::flash;
use crate::geometry::{Rect, pos2};
use crate::input::{CursorIcon, Event, Key as InputKey};
use crate::interact::Keys;
use crate::painter::Painter;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{NodeRef, WriteSignal, with_document, with_reactive_scope};
use crate::screen_reader::{Command, ScreenReader};
use crate::styled::Theme;

use panel::Summary;
use tree::{Entry, Key};

const DEFAULT_WIDTH: f32 = 320.0;
const MINIMUM_WIDTH: f32 = 200.0;
const GRIP_WIDTH: f32 = 4.0;
const GRIP_PAINT_WIDTH: f32 = 2.0;
const MINIMUM_APP_WIDTH: f32 = 480.0;
const BAR_HEIGHT: f32 = 44.0;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum InspectorTab {
    #[default]
    Beui,
    AccessKit,
    Performance,
    Simulation,
}

impl InspectorTab {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::AccessKit,
            2 => Self::Performance,
            3 => Self::Simulation,
            _ => Self::Beui,
        }
    }
}

pub(crate) struct State {
    expansion: RefCell<HashMap<Key, bool>>,
    tab: Cell<InspectorTab>,
    pub(crate) hovered: Cell<Option<NodeId>>,
    pub(crate) selected: Cell<Option<NodeId>>,
    pub(crate) picking: Cell<bool>,
    pub(crate) touch_emulation: Cell<bool>,
    pub(crate) mouse_simulation: Cell<bool>,
    pub(crate) rubber_banding: Cell<bool>,
    pub(crate) flash_changes: Cell<bool>,
    pub(crate) flash_damage: Cell<bool>,
    pub(crate) simulated_pixels_per_point: Cell<Option<f32>>,
    pub(crate) screen_reader: Cell<bool>,
    pub(crate) blur: Cell<f32>,
    pub(crate) contrast_reduction: Cell<f32>,
    pub(crate) color_vision: Cell<ColorVision>,
    commands: RefCell<Vec<Command>>,
    pub(crate) theme: Cell<Theme>,
    requested_theme: Cell<Option<Theme>>,
    reveal: Cell<Option<NodeId>>,
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
            picking: Cell::new(false),
            touch_emulation: Cell::new(ctx.touch_emulation()),
            mouse_simulation: Cell::new(ctx.mouse_simulation()),
            rubber_banding: Cell::new(true),
            flash_changes: Cell::new(false),
            flash_damage: Cell::new(false),
            simulated_pixels_per_point: Cell::new(ctx.simulated_pixels_per_point()),
            screen_reader: Cell::new(false),
            blur: Cell::new(0.0),
            contrast_reduction: Cell::new(0.0),
            color_vision: Cell::new(ColorVision::Typical),
            commands: RefCell::new(Vec::new()),
            theme: Cell::new(theme),
            requested_theme: Cell::new(None),
            reveal: Cell::new(None),
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

    fn simulate_pixels_per_point(&self, pixels_per_point: Option<f32>) {
        self.simulated_pixels_per_point.set(pixels_per_point);
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
        self.reveal.set(Some(id));
        self.touch();
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

    pub(crate) fn app_visible(&self) -> bool {
        !self.compact.get() || self.app_shown.get()
    }

    fn touch(&self) {
        self.revision.set(self.revision.get() + 1);
    }
}

pub(crate) struct Layout {
    pub(crate) bar: Rect,
    pub(crate) content: Rect,
    pub(crate) readout: Rect,
    pub(crate) panel: Rect,
    pub(crate) app_visible: bool,
}

impl Layout {
    pub(crate) fn app(rect: Rect) -> Self {
        Self {
            bar: Rect::NOTHING,
            content: rect,
            readout: Rect::NOTHING,
            panel: Rect::NOTHING,
            app_visible: true,
        }
    }
}

#[derive(Clone, Copy)]
enum Layer {
    Below,
    Above,
}

pub(crate) struct Inspector {
    pub(crate) document: Document,
    pub(crate) entries: Vec<Entry>,
    pub(crate) state: Rc<State>,
    reader: ScreenReader,
    set_keys: WriteSignal<Vec<Key>>,
    set_entries: WriteSignal<HashMap<Key, Entry>>,
    set_summary: WriteSignal<Summary>,
    set_performance: WriteSignal<panel::PerformanceSummary>,
    set_renderer: WriteSignal<panel::RendererRows>,
    set_selection: WriteSignal<Option<Key>>,
    set_reveal: WriteSignal<Option<Key>>,
    tree: NodeRef,
    bar: panel::Bar,
    pub(crate) width: f32,
    grabbed: Option<f32>,
    grip: bool,
    parked: bool,
    seen: u64,
    overlay_bounds: Cell<Rect>,
}

impl Inspector {
    pub(crate) fn new(ctx: &Context, theme: Theme) -> Self {
        let state = Rc::new(State::new(ctx, theme));
        let panel = panel::build(&state);
        let bar = panel::build_bar(&state);
        Self {
            bar,
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
            set_reveal: panel.set_reveal,
            width: DEFAULT_WIDTH,
            grabbed: None,
            grip: false,
            parked: false,
            seen: 0,
            overlay_bounds: Cell::new(Rect::NOTHING),
        }
    }

    #[cfg(test)]
    pub(crate) fn row_node(&self, index: usize) -> NodeId {
        self.find(&self.entries[index].key.test_id())
    }

    #[cfg(test)]
    pub(crate) fn find(&self, test_id: &str) -> NodeId {
        self.document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("the inspector has no node with test id {test_id:?}"))
    }

    #[cfg(test)]
    pub(crate) fn option_node(&self, test_id: &str, index: usize) -> NodeId {
        self.document.children(self.find(test_id))[index]
    }

    #[cfg(test)]
    pub(crate) fn bar_option_rect(&self, index: usize) -> Option<Rect> {
        let document = &self.bar.document;
        let bar = document.find_test_id("inspector.bar")?;
        document.node_rect(document.children(bar)[index])
    }

    #[cfg(test)]
    pub(crate) fn panel_rect(&self) -> Option<Rect> {
        self.document
            .root()
            .and_then(|root| self.document.node_rect(root))
    }

    #[cfg(test)]
    pub(crate) fn reader(&self) -> &ScreenReader {
        &self.reader
    }

    #[cfg(test)]
    pub(crate) fn focused_row(&self) -> Option<Key> {
        crate::unstyled::tree_focused::<Key>(&self.document, self.tree.get())
    }

    pub(crate) fn panel_width(&self, ctx: &Context, rect: Rect) -> f32 {
        let scale = scale(ctx);
        (self.width * scale)
            .min(rect.width() / 2.0)
            .min(rect.width() - MINIMUM_APP_WIDTH * scale)
            .max(0.0)
    }

    pub(crate) fn closed(&self) -> bool {
        self.state.closed.get()
    }

    pub(crate) fn readout_height(&self, ctx: &Context) -> f32 {
        match self.state.screen_reader.get() {
            true => ScreenReader::height() * scale(ctx),
            false => 0.0,
        }
    }

    pub(crate) fn layout(&mut self, ctx: &Context, rect: Rect) -> Layout {
        let scale = scale(ctx);
        let compact = rect.width() < (MINIMUM_APP_WIDTH + DEFAULT_WIDTH) * scale;
        self.state.compact.set(compact);
        if !compact {
            self.grab(ctx, rect);
            let (content, panel) = split(rect, self.panel_width(ctx, rect));
            return Layout {
                panel,
                ..Layout::app(content)
            };
        }
        self.grabbed = None;
        self.grip = false;
        let (bar, rect) = trim_top(rect, BAR_HEIGHT * scale);
        let app_visible = self.state.app_visible();
        Layout {
            bar,
            content: rect,
            readout: Rect::NOTHING,
            panel: if app_visible { Rect::NOTHING } else { rect },
            app_visible,
        }
    }

    pub(crate) fn intercepts(&self) -> bool {
        self.state.picking.get() || self.grabbed.is_some() || self.state.screen_reader.get()
    }

    pub(crate) fn keys(&self) -> Keys {
        if self.state.picking.get() || self.grabbed.is_some() {
            return Keys::Ignored;
        }
        match self.state.screen_reader.get() {
            true => Keys::BesideScreenReader,
            false => Keys::All,
        }
    }

    pub(crate) fn toggle_picking(&self) {
        self.state.toggle_picking();
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
            let maximum = (rect.width() / scale / 2.0)
                .min(rect.width() / scale - MINIMUM_APP_WIDTH)
                .max(MINIMUM_WIDTH);
            self.width =
                ((rect.right() - pointer.x + grabbed) / scale).clamp(MINIMUM_WIDTH, maximum);
        }
        self.grip = self.grabbed.is_some() || grip.contains(pointer);
    }

    pub(crate) fn show(
        &mut self,
        target: &mut Document,
        ctx: &Context,
        layout: &Layout,
        keyboard_interactive: bool,
    ) {
        let Layout {
            bar,
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
            ctx.scaled(scale, || {
                let keys = match keyboard_interactive {
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
        ctx.set_mouse_simulation(self.state.mouse_simulation.get());
        target.set_rubber_banding(self.state.rubber_banding.get());
        target.track_changes(self.state.flash_changes.get());
        target.track_damage(self.state.flash_damage.get());
        ctx.set_simulated_pixels_per_point(self.state.simulated_pixels_per_point.get());
        if let Some(theme) = self.state.requested_theme.take() {
            target.set_theme(theme);
            ctx.request_repaint();
        }
        if app_visible {
            self.pick(target, ctx, content);
        }
        self.release_focus(ctx);
        self.reveal();
        if app_visible {
            self.read(target, ctx, content, keyboard_interactive);
            self.paint(target, ctx, content, panel);
            let covering = self.reader.painting();
            if covering {
                self.cover(ctx, content, readout, Layer::Below);
            }
            ctx.apply_filter(self.state.filter(content));
            if covering {
                self.cover(ctx, content, readout, Layer::Above);
            }
        }
        if target.flashing() {
            ctx.request_repaint();
        }
        if self.state.revision.get() != self.seen {
            self.seen = self.state.revision.get();
            ctx.request_repaint();
        }
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
            Layer::Below => {
                let painter = ctx.painter().with_clip_rect(content.scaled(local));
                reader.paint_focus(&painter, local);
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
            InspectorTab::AccessKit => tree::collect_accesskit(target, &self.state),
            InspectorTab::Performance | InspectorTab::Simulation => Vec::new(),
        };
        let summary = self.summary(target, ctx, &entries);
        let performance = panel::PerformanceSummary::from(target.performance());
        let renderer = ctx
            .renderer_info()
            .map(|info| info.rows())
            .unwrap_or_default();
        let selection = entries
            .iter()
            .find(|entry| entry.selected)
            .map(|entry| entry.key);
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

    fn summary(&self, target: &Document, ctx: &Context, entries: &[Entry]) -> Summary {
        let selected = self.state.selected.get();
        let selection = selected
            .and_then(|id| entries.iter().find(|entry| entry.key.node() == id))
            .map_or_else(nothing_selected, entry_label);
        Summary {
            total: match self.state.tab.get() {
                InspectorTab::Beui => target.root().map_or(0, |root| tree::count(target, root)),
                InspectorTab::AccessKit => tree::accesskit_count(target),
                InspectorTab::Performance | InspectorTab::Simulation => 0,
            },
            native_pixel_ratio: native_pixel_ratio_label(ctx.native_pixels_per_point()),
            picking: self.state.picking.get(),
            selection,
            bounds: selected
                .and_then(|id| target.node_rect(id))
                .map(bounds_label)
                .unwrap_or_default(),
        }
    }

    pub(crate) fn toggle_focus(&mut self) {
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
            .map(|tree| self.document.focusables_within(tree))
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
        let Some(id) = overlay::hit(target, pointer) else {
            return;
        };
        self.state.hovered.set(Some(id));
        if ctx.input(|input| input.pointer.primary_pressed()) {
            self.state.picking.set(false);
            self.state.hovered.set(None);
            self.state.app_shown.set(false);
            self.choose(target, id);
        }
    }

    fn choose(&mut self, target: &Document, id: NodeId) {
        let path = match self.state.tab.get() {
            InspectorTab::Beui => tree::path(target, id),
            InspectorTab::AccessKit => tree::accesskit_path(target, id),
            InspectorTab::Performance | InspectorTab::Simulation => return,
        };
        if let Some((_, ancestors)) = path.split_last() {
            for key in ancestors {
                self.state.set_expanded(*key, true);
            }
        }
        self.state.select(id);
    }

    fn reveal(&mut self) {
        let Some(id) = self.state.reveal.get() else {
            return;
        };
        let key = match self.state.tab.get() {
            InspectorTab::Beui => Key::Node(id),
            InspectorTab::AccessKit => Key::AccessKit(id),
            InspectorTab::Performance | InspectorTab::Simulation => return,
        };
        if !self.entries.iter().any(|entry| entry.key == key) {
            return;
        }
        self.state.reveal.set(None);
        let Self {
            document,
            set_reveal,
            ..
        } = self;
        with_reactive_scope(document, || set_reveal.set(Some(key)));
        with_reactive_scope(document, || set_reveal.set(None));
        self.state.touch();
    }

    fn paint(&self, target: &Document, ctx: &Context, content: Rect, panel: Rect) {
        let scale = scale(ctx);
        let local = scale.recip();
        ctx.scaled(scale, || {
            let painted = ctx.measure_paint(|| {
                let painter = ctx.painter().with_clip_rect(content.scaled(local));
                flashes(&painter, target, local);
                let hovered = self.state.hovered.get();
                let selected = self.state.selected.get();
                if let Some(id) = selected.filter(|id| Some(*id) != hovered) {
                    overlay::highlight(&painter, target, id, false, local);
                }
                if let Some(id) = hovered {
                    overlay::highlight(&painter, target, id, true, local);
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

pub(crate) fn scale(ctx: &Context) -> f32 {
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
