use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{ActionRequest, TreeUpdate};

use crate::accessibility::{self, Fragment};
use crate::damage::{self, Region};
use crate::display::{Display, Layer};
use crate::filter::Filter;
use crate::font::{FontBackend, FontId, Fonts, Galley, TextLayout};
use crate::geometry::{Rect, pos2};
use crate::input::{CursorIcon, Event, ImeArea, InputState, RawInput};
use crate::node::NodeId;
use crate::paint::{Item, Recorded};
use crate::painter::{Entry, Painter, Shape};
use crate::screen_simulation::ScreenSimulation;

#[derive(Clone)]
pub struct Context {
    inner: Rc<Inner>,
}

struct Inner {
    fonts: RefCell<Fonts>,
    input: RefCell<InputState>,
    layers: RefCell<Vec<Layer>>,
    top_shapes: RefCell<Vec<Shape>>,
    filter: Cell<Option<(Filter, usize)>>,
    paint_stack: RefCell<Vec<PaintFrame>>,
    space_read: Cell<bool>,
    damage: RefCell<Vec<Rect>>,
    moves: RefCell<Vec<(Moved, Vec<Rc<Display>>)>>,
    test_ids: RefCell<HashMap<String, Rect>>,
    ambiguous_test_ids: RefCell<HashSet<String>>,
    copied_text: RefCell<Option<String>>,
    paste_requested: Cell<bool>,
    cursor_icon: Cell<CursorIcon>,
    ime: Cell<Option<ImeArea>>,
    fullscreen: Cell<Option<bool>>,
    close_requested: Cell<bool>,
    handles_back: Cell<bool>,
    pointer_locked: Cell<bool>,
    touch_emulation: Cell<bool>,
    input_simulation: RefCell<Option<Box<dyn InputSimulation>>>,
    simulation_area: Cell<(Rect, f32)>,
    mouse_viewport: Cell<Option<Rect>>,
    pixels_per_point: Cell<f32>,
    native_pixels_per_point: Cell<f32>,
    simulated_pixels_per_point: Cell<Option<f32>>,
    screen_simulation: Cell<Option<ScreenSimulation>>,
    screen_scale: Cell<f32>,
    zoom: Cell<f32>,
    repaint: Cell<bool>,
    repaint_after: Cell<Duration>,
    previous: RefCell<Option<Previous>>,
    accessibility: RefCell<Vec<Fragment>>,
    accessibility_actions: RefCell<Vec<ActionRequest>>,
    accessibility_active: Cell<bool>,
    accessibility_known: RefCell<HashSet<u32>>,
    accessibility_published: RefCell<HashSet<u32>>,
    test_ids_published: Cell<bool>,
    renderer_info: RefCell<Option<RendererInfo>>,
}

pub trait InputSimulation: Any {
    fn enabled(&self) -> bool;

    fn set_enabled(&mut self, enabled: bool);

    fn translate(&mut self, raw: RawInput) -> (RawInput, Option<Duration>);

    fn measure(&mut self, viewport: Rect, scale: f32);

    fn reserved(&self) -> f32;

    fn painting(&self) -> bool;

    fn paint(&mut self, painter: &Painter, icon: CursorIcon) -> Rect;

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RendererInfo {
    pub rows: Vec<(&'static str, String)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Moved {
    pub from: Rect,
    pub by: crate::geometry::Vec2,
}

impl Moved {
    pub fn to(&self) -> Rect {
        self.from.translate(self.by)
    }
}

pub struct FrameOutput {
    pub layers: Rc<Vec<Layer>>,
    pub filter: Option<(Filter, usize)>,
    test_ids: HashMap<String, Rect>,
    ambiguous_test_ids: HashSet<String>,
    pub cursor_icon: CursorIcon,
    pub ime: Option<ImeArea>,
    pub fullscreen: Option<bool>,
    pub close_requested: bool,
    pub handles_back: bool,
    pub pointer_locked: bool,
    pub copied_text: Option<String>,
    pub paste_requested: bool,
    pub repaint: bool,
    pub repaint_after: Duration,
    pub changed: bool,
    pub damage: Region,
    pub moved: Option<Moved>,
    accessibility: Vec<Fragment>,
    pixels_per_point: f32,
}

impl FrameOutput {
    pub fn shapes(&self) -> Vec<Shape> {
        crate::display::flatten(&self.layers)
    }

    pub fn filtered_shapes(&self) -> (Vec<Shape>, Option<usize>) {
        let boundary = self.filter.map(|(_, boundary)| boundary);
        let mut shapes = Vec::new();
        let mut filtered = None;
        for (index, layer) in self.layers.iter().enumerate() {
            if boundary == Some(index) {
                filtered = Some(shapes.len());
            }
            layer.flatten(&mut shapes);
        }
        if boundary.is_some_and(|boundary| boundary >= self.layers.len()) {
            filtered = Some(shapes.len());
        }
        (shapes, filtered)
    }

    pub fn filter(&self) -> Option<Filter> {
        self.filter.map(|(filter, _)| filter)
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    pub fn damaged(&self) -> Option<Region> {
        let damage = self.unmoved();
        (!damage.is_empty()).then_some(damage)
    }

    pub fn moved(&self) -> Option<Moved> {
        self.moved
    }

    pub fn unmoved(&self) -> Region {
        match self.moved {
            Some(moved) => self.damage.union(moved.to().into()),
            None => self.damage,
        }
    }

    pub fn damage(&self) -> Option<Rect> {
        let bounds = self.unmoved().bounds();
        bounds.is_positive().then_some(bounds)
    }

    pub fn test_id_rect(&self, test_id: &str) -> Option<Rect> {
        if self.ambiguous_test_ids.contains(test_id) {
            panic!("test id {test_id:?} names more than one node on screen; give each its own id");
        }
        self.test_ids.get(test_id).copied()
    }

    pub fn accessibility_tree(&self, title: &str, viewport: crate::geometry::Vec2) -> TreeUpdate {
        accessibility::tree_update(
            title,
            viewport,
            self.pixels_per_point,
            self.accessibility.clone(),
        )
    }
}

impl Context {
    pub fn new(fonts: impl FontBackend + 'static) -> Self {
        Self {
            inner: Rc::new(Inner {
                fonts: RefCell::new(Fonts::new(fonts)),
                input: RefCell::new(InputState::default()),
                layers: RefCell::new(Vec::new()),
                top_shapes: RefCell::new(Vec::new()),
                filter: Cell::new(None),
                paint_stack: RefCell::new(Vec::new()),
                space_read: Cell::new(false),
                damage: RefCell::new(Vec::new()),
                moves: RefCell::new(Vec::new()),
                test_ids: RefCell::new(HashMap::new()),
                ambiguous_test_ids: RefCell::new(HashSet::new()),
                copied_text: RefCell::new(None),
                paste_requested: Cell::new(false),
                cursor_icon: Cell::new(CursorIcon::Default),
                ime: Cell::new(None),
                fullscreen: Cell::new(None),
                close_requested: Cell::new(false),
                handles_back: Cell::new(false),
                pointer_locked: Cell::new(false),
                touch_emulation: Cell::new(false),
                input_simulation: RefCell::new(None),
                simulation_area: Cell::new((Rect::NOTHING, 1.0)),
                mouse_viewport: Cell::new(None),
                pixels_per_point: Cell::new(1.0),
                native_pixels_per_point: Cell::new(1.0),
                simulated_pixels_per_point: Cell::new(None),
                screen_simulation: Cell::new(None),
                screen_scale: Cell::new(1.0),
                zoom: Cell::new(1.0),
                repaint: Cell::new(false),
                repaint_after: Cell::new(Duration::MAX),
                previous: RefCell::new(None),
                accessibility: RefCell::new(Vec::new()),
                accessibility_actions: RefCell::new(Vec::new()),
                accessibility_active: Cell::new(true),
                accessibility_known: RefCell::new(HashSet::new()),
                accessibility_published: RefCell::new(HashSet::new()),
                test_ids_published: Cell::new(true),
                renderer_info: RefCell::new(None),
            }),
        }
    }

    pub fn set_renderer_info(&self, info: RendererInfo) {
        *self.inner.renderer_info.borrow_mut() = Some(info);
    }

    pub fn renderer_info(&self) -> Option<RendererInfo> {
        self.inner.renderer_info.borrow().clone()
    }

    pub fn set_accessibility_active(&self, active: bool) {
        self.inner.accessibility_active.set(active);
        if !active {
            self.reset_accessibility();
        }
    }

    pub fn reset_accessibility(&self) {
        self.inner.accessibility_known.borrow_mut().clear();
        self.inner.accessibility_published.borrow_mut().clear();
    }

    pub fn accessibility_known(&self, document: u32) -> bool {
        self.inner.accessibility_known.borrow().contains(&document)
    }

    pub fn accessibility_active(&self) -> bool {
        self.inner.accessibility_active.get()
    }

    pub fn set_test_ids_published(&self, published: bool) {
        self.inner.test_ids_published.set(published);
    }

    pub fn test_ids_published(&self) -> bool {
        self.inner.test_ids_published.get()
    }

    pub fn begin_frame(&self, raw: RawInput) {
        self.apply_pixels_per_point();
        self.inner.repaint.set(false);
        self.inner.repaint_after.set(Duration::MAX);
        self.inner.mouse_viewport.set(None);
        let (raw, wake) = match self.inner.input_simulation.borrow_mut().as_mut() {
            Some(simulation) => simulation.translate(raw),
            None => (raw, None),
        };
        if let Some(delay) = wake {
            self.request_repaint_after(delay);
        }
        self.inner.input.borrow_mut().begin_frame(raw);
        let held = self.inner.input.borrow().long_press_due(Instant::now());
        if let Some(delay) = held {
            self.request_repaint_after(delay);
        }
        self.inner.layers.borrow_mut().clear();
        self.inner.top_shapes.borrow_mut().clear();
        self.inner.filter.set(None);
        self.inner.paint_stack.borrow_mut().clear();
        self.inner.damage.borrow_mut().clear();
        self.inner.moves.borrow_mut().clear();
        self.inner.test_ids.borrow_mut().clear();
        self.inner.ambiguous_test_ids.borrow_mut().clear();
        self.inner.copied_text.borrow_mut().take();
        self.inner.paste_requested.set(false);
        self.inner.cursor_icon.set(CursorIcon::Default);
        self.inner.ime.set(None);
        self.inner.fullscreen.set(None);
        self.inner.close_requested.set(false);
        self.inner.handles_back.set(false);
        self.inner.accessibility.borrow_mut().clear();
        let published = std::mem::take(&mut *self.inner.accessibility_published.borrow_mut());
        *self.inner.accessibility_known.borrow_mut() = published;
    }

    pub fn end_frame(&self) -> FrameOutput {
        if let Some(viewport) = self.inner.mouse_viewport.take() {
            self.paint_mouse_simulation(viewport);
        }
        self.flush_top();
        let layers = Rc::new(std::mem::take(&mut *self.inner.layers.borrow_mut()));
        let scale = self.pixels_per_point();
        let filter = self.inner.filter.take();
        let moved = self.settle_moves(&layers, scale);
        let reported = std::mem::take(&mut *self.inner.damage.borrow_mut());
        let damage = reported
            .iter()
            .fold(Region::NOTHING, |region, rect| region.union((*rect).into()));
        let mut previous = self.inner.previous.borrow_mut();
        let changed = match previous.as_ref() {
            None => true,
            Some(old) if old.pixels_per_point != scale || old.filter != filter => true,
            Some(old) => {
                let same = old.layers.len() == layers.len()
                    && old
                        .layers
                        .iter()
                        .zip(layers.iter())
                        .all(|(old, new)| old.same(new));
                if reported.is_empty() && !same {
                    debug_assert!(
                        crate::display::flatten(&old.layers) == crate::display::flatten(&layers),
                        "a frame that reported no damage changed the shapes it painted"
                    );
                }
                (!reported.is_empty() || moved.is_some()) && !same
            }
        };
        *previous = Some(Previous {
            layers: Rc::clone(&layers),
            pixels_per_point: scale,
            filter,
        });
        FrameOutput {
            layers,
            filter,
            damage,
            moved,
            test_ids: std::mem::take(&mut *self.inner.test_ids.borrow_mut()),
            ambiguous_test_ids: std::mem::take(&mut *self.inner.ambiguous_test_ids.borrow_mut()),
            copied_text: self.inner.copied_text.borrow_mut().take(),
            paste_requested: self.inner.paste_requested.replace(false),
            changed,
            repaint_after: self.inner.repaint_after.get(),
            cursor_icon: self.inner.cursor_icon.get(),
            ime: self.inner.ime.get(),
            fullscreen: self.inner.fullscreen.get(),
            close_requested: self.inner.close_requested.get(),
            handles_back: self.inner.handles_back.get(),
            pointer_locked: self.inner.pointer_locked.get(),
            repaint: self.inner.repaint.get(),
            accessibility: std::mem::take(&mut *self.inner.accessibility.borrow_mut()),
            pixels_per_point: scale,
        }
    }

    pub fn run(&self, raw: RawInput, frame: impl FnOnce(&Self)) -> FrameOutput {
        self.begin_frame(raw);
        frame(self);
        self.end_frame()
    }

    pub fn input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R {
        reader(&self.inner.input.borrow())
    }

    pub fn retain_events(&self, keep: impl FnMut(&Event) -> bool) {
        self.inner.input.borrow_mut().events.retain(keep);
    }

    pub fn set_ime_area(&self, area: Option<ImeArea>) {
        self.inner.ime.set(area);
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.inner.fullscreen.set(Some(fullscreen));
    }

    pub fn close_window(&self) {
        self.inner.close_requested.set(true);
    }

    pub fn handle_back(&self) {
        self.inner.handles_back.set(true);
    }

    pub fn painter(&self) -> Painter {
        Painter::new(self.clone(), Rect::EVERYTHING)
    }

    pub fn copy_text(&self, text: String) {
        *self.inner.copied_text.borrow_mut() = Some(text);
    }

    pub fn request_paste(&self) {
        self.inner.paste_requested.set(true);
    }

    pub fn set_pointer_locked(&self, locked: bool) {
        if self.inner.pointer_locked.replace(locked) != locked {
            self.request_repaint();
        }
    }

    pub fn pointer_locked(&self) -> bool {
        self.inner.pointer_locked.get()
    }

    pub fn set_cursor_icon(&self, cursor_icon: CursorIcon) {
        self.inner.cursor_icon.set(cursor_icon);
    }

    pub fn cursor_icon(&self) -> CursorIcon {
        self.inner.cursor_icon.get()
    }

    pub fn touch_emulation(&self) -> bool {
        self.inner.touch_emulation.get()
    }

    pub fn set_touch_emulation(&self, enabled: bool) {
        self.inner.touch_emulation.set(enabled);
    }

    pub fn mouse_simulation(&self) -> bool {
        self.inner
            .input_simulation
            .borrow()
            .as_ref()
            .is_some_and(|simulation| simulation.enabled())
    }

    pub fn set_input_simulation(&self, mut simulation: Box<dyn InputSimulation>) {
        let (viewport, scale) = self.inner.simulation_area.get();
        simulation.measure(viewport, scale);
        *self.inner.input_simulation.borrow_mut() = Some(simulation);
    }

    pub fn input_simulation_mut<R>(
        &self,
        change: impl FnOnce(&mut dyn InputSimulation) -> R,
    ) -> Option<R> {
        let mut simulation = self.inner.input_simulation.borrow_mut();
        simulation.as_deref_mut().map(change)
    }

    pub fn input_simulation_as<T: 'static, R>(&self, read: impl FnOnce(&T) -> R) -> Option<R> {
        let simulation = self.inner.input_simulation.borrow();
        let simulation = simulation.as_deref()?.as_any().downcast_ref::<T>()?;
        Some(read(simulation))
    }

    pub fn input_simulation_as_mut<T: 'static, R>(
        &self,
        change: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        let mut simulation = self.inner.input_simulation.borrow_mut();
        let simulation = simulation
            .as_deref_mut()?
            .as_any_mut()
            .downcast_mut::<T>()?;
        Some(change(simulation))
    }

        pub fn measure_mouse_simulation(&self, viewport: Rect) -> f32 {
        let scale = self.native_pixels_per_point() / self.pixels_per_point();
        self.inner.simulation_area.set((viewport, scale));
        let mut simulation = self.inner.input_simulation.borrow_mut();
        let Some(simulation) = simulation.as_mut() else {
            return 0.0;
        };
        simulation.measure(viewport, scale);
        simulation.reserved()
    }

    pub fn show_mouse_simulation(&self, viewport: Rect) {
        self.inner.mouse_viewport.set(Some(viewport));
    }

    fn paint_mouse_simulation(&self, viewport: Rect) {
        let scale = self.native_pixels_per_point() / self.pixels_per_point();
        let Some(mut simulation) = self.inner.input_simulation.borrow_mut().take() else {
            return;
        };
        if simulation.painting() {
            let icon = self.cursor_icon();
            self.scaled(scale, || {
                let painter = self
                    .painter()
                    .with_clip_rect(viewport.scaled(scale.recip()));
                let damage = simulation.paint(&painter, icon);
                self.report_damage(damage);
            });
        }
        *self.inner.input_simulation.borrow_mut() = Some(simulation);
    }

    pub fn request_repaint(&self) {
        self.inner.repaint.set(true);
        self.request_repaint_after(Duration::ZERO);
    }

    pub fn request_repaint_after(&self, delay: Duration) {
        self.inner
            .repaint_after
            .set(self.inner.repaint_after.get().min(delay));
        if let Some(frame) = self.inner.paint_stack.borrow_mut().last_mut() {
            frame.delay = frame.delay.min(delay);
        }
    }

    pub fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }

    pub fn measure_paint(&self, paint: impl FnOnce()) -> Rect {
        self.push_paint_frame(None);
        paint();
        self.pop_paint_frame().bounds
    }

    pub fn note_space_read(&self) {
        self.inner.space_read.set(true);
    }

    pub fn swap_space_read(&self, read: bool) -> bool {
        self.inner.space_read.replace(read)
    }

    pub fn enter_paint(&self, id: NodeId) {
        self.push_paint_frame(Some(id));
    }

    fn push_paint_frame(&self, id: Option<NodeId>) {
        let frame = PaintFrame {
            id,
            items: Vec::new(),
            own: Rect::NOTHING,
            bounds: Rect::NOTHING,
            delay: Duration::MAX,
            outer_delay: self.inner.repaint_after.replace(Duration::MAX),
        };
        self.inner.paint_stack.borrow_mut().push(frame);
    }

    fn pop_paint_frame(&self) -> PaintFrame {
        let frame = self
            .inner
            .paint_stack
            .borrow_mut()
            .pop()
            .expect("a painted node was entered");
        let delay = self.inner.repaint_after.get();
        self.inner.repaint_after.set(frame.outer_delay.min(delay));
        frame
    }

    pub fn exit_paint(&self) -> Recorded {
        let frame = self.pop_paint_frame();
        Recorded {
            items: frame.items,
            own: frame.own,
            bounds: frame.bounds,
            deadline: (frame.delay < Duration::MAX).then(|| Instant::now() + frame.delay),
            reads: false,
        }
    }

    pub fn paint_child(&self, id: NodeId, entry: Entry, bounds: Rect) {
        if let Some(frame) = self.inner.paint_stack.borrow_mut().last_mut() {
            frame.bounds = frame.bounds.union(bounds);
            if frame.id.is_some() {
                frame.items.push(Item::Child(id, entry));
            }
        }
    }

    pub fn report_move(&self, moved: Moved, roots: Vec<Rc<Display>>) {
        self.inner.moves.borrow_mut().push((moved, roots));
    }

    fn settle_moves(&self, layers: &[Layer], pixels_per_point: f32) -> Option<Moved> {
        let moves = std::mem::take(&mut *self.inner.moves.borrow_mut());
        let whole = |value: f32| {
            ((value * pixels_per_point).round() - value * pixels_per_point).abs() < 0.01
        };
        let settled = match moves.as_slice() {
            [(moved, roots)]
                if whole(moved.by.x)
                    && whole(moved.by.y)
                    && layers.iter().all(|layer| {
                        let reach = match layer {
                            Layer::Shape(shape) => damage::bounds(shape),
                            Layer::Display {
                                display,
                                entry,
                                scale,
                                clip,
                            } => {
                                if roots.iter().any(|root| Rc::ptr_eq(root, display)) {
                                    return true;
                                }
                                entry.place(display.bounds).scaled(*scale).intersect(*clip)
                            }
                        };
                        !reach.intersects(moved.from.union(moved.to()))
                    }) =>
            {
                Some(*moved)
            }
            _ => None,
        };
        if settled.is_none() {
            for (moved, _) in &moves {
                self.report_damage(moved.to());
            }
        }
        settled
    }

    fn unmove(&self, moves: usize) {
        let unmoved: Vec<(Moved, Vec<Rc<Display>>)> =
            self.inner.moves.borrow_mut().drain(moves..).collect();
        for (moved, _) in unmoved {
            self.report_damage(moved.to());
        }
    }

    pub fn report_damage(&self, rect: Rect) {
        if rect.is_positive() {
            self.inner.damage.borrow_mut().push(rect);
        }
    }

    pub fn show_painting(&self, painting: &[(Rc<Display>, Entry)]) {
        self.inner
            .layers
            .borrow_mut()
            .extend(painting.iter().map(|(display, entry)| Layer::Display {
                display: Rc::clone(display),
                entry: *entry,
                scale: 1.0,
                clip: Rect::EVERYTHING,
            }));
    }

    pub fn publish_test_id(&self, test_id: &str, rect: Rect) {
        if !self.inner.test_ids_published.get() {
            return;
        }
        self.inner
            .test_ids
            .borrow_mut()
            .insert(test_id.to_owned(), rect);
    }

    pub fn publish_ambiguous_test_id(&self, test_id: &str) {
        if self.inner.test_ids_published.get() {
            self.inner
                .ambiguous_test_ids
                .borrow_mut()
                .insert(test_id.to_owned());
        }
    }

    pub fn publish_accessibility(&self, document: u32, fragment: Fragment) {
        self.inner
            .accessibility_published
            .borrow_mut()
            .insert(document);
        self.inner.accessibility.borrow_mut().push(fragment);
    }

    pub fn accessibility_action(&self, request: ActionRequest) {
        self.inner.accessibility_actions.borrow_mut().push(request);
        self.request_repaint();
    }

    pub fn take_accessibility_actions(&self, document_id: u32) -> Vec<ActionRequest> {
        let mut actions = self.inner.accessibility_actions.borrow_mut();
        let all = std::mem::take(&mut *actions);
        let (matched, remaining) = all.into_iter().partition(|request| {
            crate::accessibility::document_of(request.target_node) == document_id as u64
        });
        *actions = remaining;
        matched
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.inner.pixels_per_point.get()
    }

    pub fn set_long_press_delay(&self, delay: Duration) {
        self.inner.input.borrow_mut().long_press_delay = delay;
    }

    pub fn set_pixels_per_point(&self, pixels_per_point: f32) {
        self.inner.native_pixels_per_point.set(pixels_per_point);
        self.apply_pixels_per_point();
    }

    pub fn native_pixels_per_point(&self) -> f32 {
        self.inner.native_pixels_per_point.get()
    }

    pub fn simulated_pixels_per_point(&self) -> Option<f32> {
        self.inner.simulated_pixels_per_point.get()
    }

    pub fn set_simulated_pixels_per_point(&self, pixels_per_point: Option<f32>) {
        self.inner.simulated_pixels_per_point.set(pixels_per_point);
    }

    pub fn screen_simulation(&self) -> Option<ScreenSimulation> {
        self.inner.screen_simulation.get()
    }

    pub fn screen_scale(&self) -> f32 {
        self.inner.screen_scale.get()
    }

    pub fn set_screen_scale(&self, scale: f32) {
        self.inner.screen_scale.set(scale);
    }

    pub fn screen_input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R {
        let scale = self.screen_scale();
        if scale == 1.0 {
            return self.input(reader);
        }
        reader(&self.inner.input.borrow().scaled(scale.recip()))
    }

    pub fn set_screen_simulation(&self, simulation: Option<ScreenSimulation>) {
        self.inner.screen_simulation.set(simulation);
    }

    pub fn clipped<R>(&self, clip: Rect, content: impl FnOnce() -> R) -> R {
        let layers = self.inner.layers.borrow().len();
        let damage = self.inner.damage.borrow().len();
        let moves = self.inner.moves.borrow().len();
        let result = content();
        self.unmove(moves);
        for layer in self.inner.layers.borrow_mut().iter_mut().skip(layers) {
            match layer {
                Layer::Shape(shape) => {
                    let bounds = shape_clip(shape);
                    *bounds = bounds.intersect(clip);
                }
                Layer::Display { clip: held, .. } => *held = held.intersect(clip),
            }
        }
        for rect in self.inner.damage.borrow_mut().iter_mut().skip(damage) {
            *rect = rect.intersect(clip);
        }
        result
    }

    pub fn zoom_factor(&self) -> f32 {
        self.inner.zoom.get()
    }

    pub fn set_zoom_factor(&self, zoom: f32) {
        if zoom.is_finite() && zoom > 0.0 && zoom != self.inner.zoom.get() {
            self.inner.zoom.set(zoom);
            self.request_repaint();
        }
    }

    fn apply_pixels_per_point(&self) {
        let pixels_per_point = self
            .simulated_pixels_per_point()
            .unwrap_or_else(|| self.native_pixels_per_point() * self.zoom_factor());
        self.inner.pixels_per_point.set(pixels_per_point);
    }

    pub fn scaled<R>(&self, scale: f32, content: impl FnOnce() -> R) -> R {
        if scale == 1.0 {
            return content();
        }
        let pixels_per_point = self.pixels_per_point();
        self.inner.pixels_per_point.set(pixels_per_point * scale);
        let input = self
            .inner
            .input
            .replace_with(|input| input.scaled(scale.recip()));
        let layers = self.inner.layers.borrow().len();
        let damage = self.inner.damage.borrow().len();
        let moves = self.inner.moves.borrow().len();
        let fragments = self.inner.accessibility.borrow().len();
        let test_ids = self.inner.test_ids.take();
        let ime = self.inner.ime.take();
        let result = content();
        self.unmove(moves);
        let scaled_ime = self.inner.ime.replace(ime);
        if let Some(area) = scaled_ime {
            self.inner.ime.set(Some(ImeArea {
                rect: area.rect.scaled(scale),
                cursor: area.cursor.scaled(scale),
            }));
        }
        self.inner.input.replace(input);
        self.inner.pixels_per_point.set(pixels_per_point);
        for layer in self.inner.layers.borrow_mut().iter_mut().skip(layers) {
            match layer {
                Layer::Shape(shape) => scale_shape(shape, scale),
                Layer::Display {
                    scale: held, clip, ..
                } => {
                    *held *= scale;
                    *clip = clip.scaled(scale);
                }
            }
        }
        for rect in self.inner.damage.borrow_mut().iter_mut().skip(damage) {
            *rect = rect.scaled(scale);
        }
        for fragment in self
            .inner
            .accessibility
            .borrow_mut()
            .iter_mut()
            .skip(fragments)
        {
            fragment.scale(scale);
        }
        let scaled = self.inner.test_ids.replace(test_ids);
        self.inner.test_ids.borrow_mut().extend(
            scaled
                .into_iter()
                .map(|(test_id, rect)| (test_id, rect.scaled(scale))),
        );
        result
    }

    pub fn fonts_generation(&self) -> u64 {
        self.inner.fonts.borrow_mut().generation()
    }

    pub fn layout(&self, text: &str, font: FontId, layout: TextLayout) -> Galley {
        self.inner
            .fonts
            .borrow_mut()
            .layout(text, font, layout, self.pixels_per_point())
    }

    pub fn push(&self, shape: Shape) {
        if let Some(shape) = self.record(shape, Item::Main) {
            self.inner.layers.borrow_mut().push(Layer::Shape(shape));
        }
    }

    pub fn push_top(&self, shape: Shape) {
        if let Some(shape) = self.record(shape, Item::Top) {
            self.inner.top_shapes.borrow_mut().push(shape);
        }
    }

    fn record(&self, shape: Shape, item: fn(Shape) -> Item) -> Option<Shape> {
        let mut stack = self.inner.paint_stack.borrow_mut();
        let Some(frame) = stack.last_mut() else {
            return Some(shape);
        };
        let bounds = damage::bounds(&shape);
        frame.own = frame.own.union(bounds);
        frame.bounds = frame.bounds.union(bounds);
        if frame.id.is_none() {
            return Some(shape);
        }
        if bounds.is_positive() || matches!(shape, Shape::Drawing { .. }) {
            frame.items.push(item(shape));
        }
        None
    }

    pub fn apply_filter(&self, filter: Filter) {
        if filter.changes_nothing() || !filter.region.is_positive() {
            return;
        }
        let boundary = self.inner.layers.borrow().len();
        self.inner.filter.set(Some((filter, boundary)));
    }

    fn flush_top(&self) {
        let top = std::mem::take(&mut *self.inner.top_shapes.borrow_mut());
        self.inner
            .layers
            .borrow_mut()
            .extend(top.into_iter().map(Layer::Shape));
    }
}

struct Previous {
    layers: Rc<Vec<Layer>>,
    pixels_per_point: f32,
    filter: Option<(Filter, usize)>,
}

struct PaintFrame {
    id: Option<NodeId>,
    items: Vec<Item>,
    own: Rect,
    bounds: Rect,
    delay: Duration,
    outer_delay: Duration,
}

pub fn shape_clip(shape: &mut Shape) -> &mut Rect {
    match shape {
        Shape::Rect { clip, .. }
        | Shape::Text { clip, .. }
        | Shape::Line { clip, .. }
        | Shape::Image { clip, .. }
        | Shape::Punch { clip, .. }
        | Shape::Drawing { clip, .. } => clip,
    }
}

pub fn scale_shape(shape: &mut Shape, scale: f32) {
    match shape {
        Shape::Rect {
            rect,
            corner_radius,
            stroke_width,
            rotation,
            clip,
            ..
        } => {
            *rect = rect.scaled(scale);
            *corner_radius *= scale;
            *stroke_width *= scale;
            *rotation = rotation.scaled(scale);
            *clip = clip.scaled(scale);
        }
        Shape::Text {
            origin,
            rotation,
            clip,
            ..
        } => {
            *origin = pos2(origin.x * scale, origin.y * scale);
            *rotation = rotation.scaled(scale);
            *clip = clip.scaled(scale);
        }
        Shape::Line {
            from,
            to,
            width,
            clip,
            ..
        } => {
            *from = pos2(from.x * scale, from.y * scale);
            *to = pos2(to.x * scale, to.y * scale);
            *width *= scale;
            *clip = clip.scaled(scale);
        }
        Shape::Image {
            rect,
            corner_radius,
            rotation,
            clip,
            ..
        }
        | Shape::Punch {
            rect,
            corner_radius,
            rotation,
            clip,
        } => {
            *rect = rect.scaled(scale);
            *corner_radius *= scale;
            *rotation = rotation.scaled(scale);
            *clip = clip.scaled(scale);
        }
        Shape::Drawing { rect, clip, .. } => {
            *rect = rect.scaled(scale);
            *clip = clip.scaled(scale);
        }
    }
}
