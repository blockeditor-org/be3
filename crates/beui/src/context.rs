use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{ActionRequest, TreeUpdate};

use crate::accessibility::{self, Fragment};
use crate::damage;
use crate::filter::Filter;
use crate::font::{FontId, FontSources, Fonts, Galley, TextLayout};
use crate::geometry::{Rect, pos2};
use crate::input::{CursorIcon, InputState, RawInput};
use crate::mouse_simulation::MouseSimulation;
use crate::node::NodeId;
use crate::paint::Painted;
use crate::painter::{Painter, Shape};

#[derive(Clone)]
pub struct Context {
    inner: Rc<Inner>,
}

struct Inner {
    fonts: RefCell<Fonts>,
    input: RefCell<InputState>,
    shapes: RefCell<Vec<Shape>>,
    top_shapes: RefCell<Vec<Shape>>,
    filter: Cell<Option<(Filter, usize)>>,
    capture_base: Cell<usize>,
    paint_stack: RefCell<Vec<PaintFrame>>,
    deadlines: RefCell<Vec<NodeId>>,
    damage: RefCell<Vec<Rect>>,
    test_ids: RefCell<HashMap<String, Rect>>,
    copied_text: RefCell<Option<String>>,
    paste_requested: Cell<bool>,
    cursor_icon: Cell<CursorIcon>,
    pointer_locked: Cell<bool>,
    touch_emulation: Cell<bool>,
    mouse_simulation: RefCell<MouseSimulation>,
    pixels_per_point: Cell<f32>,
    native_pixels_per_point: Cell<f32>,
    simulated_pixels_per_point: Cell<Option<f32>>,
    repaint: Cell<bool>,
    repaint_after: Cell<Duration>,
    previous: RefCell<Option<Previous>>,
    accessibility: RefCell<Vec<Fragment>>,
    accessibility_actions: RefCell<Vec<ActionRequest>>,
    accessibility_active: Cell<bool>,
    test_ids_published: Cell<bool>,
}

pub struct FrameOutput {
    pub(crate) shapes: Rc<Vec<Shape>>,
    pub(crate) filter: Option<(Filter, usize)>,
    test_ids: HashMap<String, Rect>,
    pub cursor_icon: CursorIcon,
    pub pointer_locked: bool,
    pub copied_text: Option<String>,
    pub paste_requested: bool,
    pub repaint: bool,
    pub repaint_after: Duration,
    pub changed: bool,
    damage: Rect,
    accessibility: Vec<Fragment>,
    pixels_per_point: f32,
}

impl FrameOutput {
    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    pub fn filter(&self) -> Option<Filter> {
        self.filter.map(|(filter, _)| filter)
    }

    pub fn filtered_shapes(&self) -> Option<usize> {
        self.filter.map(|(_, boundary)| boundary)
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    pub fn damage(&self) -> Option<Rect> {
        self.damage.is_positive().then_some(self.damage)
    }

    pub fn test_id_rect(&self, test_id: &str) -> Option<Rect> {
        self.test_ids.get(test_id).copied()
    }

    pub fn accessibility_tree(&self, title: &str, viewport: crate::Vec2) -> TreeUpdate {
        accessibility::tree_update(
            title,
            viewport,
            self.pixels_per_point,
            self.accessibility.clone(),
        )
    }
}

impl Context {
    pub fn new() -> Self {
        Self::with_fonts(&FontSources::default())
    }

    pub fn with_fonts(sources: &FontSources) -> Self {
        Self {
            inner: Rc::new(Inner {
                fonts: RefCell::new(Fonts::new(sources)),
                input: RefCell::new(InputState::default()),
                shapes: RefCell::new(Vec::new()),
                top_shapes: RefCell::new(Vec::new()),
                filter: Cell::new(None),
                capture_base: Cell::new(0),
                paint_stack: RefCell::new(Vec::new()),
                deadlines: RefCell::new(Vec::new()),
                damage: RefCell::new(Vec::new()),
                test_ids: RefCell::new(HashMap::new()),
                copied_text: RefCell::new(None),
                paste_requested: Cell::new(false),
                cursor_icon: Cell::new(CursorIcon::Default),
                pointer_locked: Cell::new(false),
                touch_emulation: Cell::new(false),
                mouse_simulation: RefCell::new(MouseSimulation::default()),
                pixels_per_point: Cell::new(1.0),
                native_pixels_per_point: Cell::new(1.0),
                simulated_pixels_per_point: Cell::new(None),
                repaint: Cell::new(false),
                repaint_after: Cell::new(Duration::MAX),
                previous: RefCell::new(None),
                accessibility: RefCell::new(Vec::new()),
                accessibility_actions: RefCell::new(Vec::new()),
                accessibility_active: Cell::new(true),
                test_ids_published: Cell::new(true),
            }),
        }
    }

    pub fn set_accessibility_active(&self, active: bool) {
        self.inner.accessibility_active.set(active);
    }

    pub(crate) fn accessibility_active(&self) -> bool {
        self.inner.accessibility_active.get()
    }

    pub fn set_test_ids_published(&self, published: bool) {
        self.inner.test_ids_published.set(published);
    }

    pub(crate) fn test_ids_published(&self) -> bool {
        self.inner.test_ids_published.get()
    }

    pub fn begin_frame(&self, raw: RawInput) {
        self.apply_pixels_per_point();
        self.inner.repaint.set(false);
        self.inner.repaint_after.set(Duration::MAX);
        let (raw, wake) = self.inner.mouse_simulation.borrow_mut().translate(raw);
        if let Some(delay) = wake {
            self.request_repaint_after(delay);
        }
        self.inner.input.borrow_mut().begin_frame(raw);
        self.inner.shapes.borrow_mut().clear();
        self.inner.top_shapes.borrow_mut().clear();
        self.inner.filter.set(None);
        self.inner.paint_stack.borrow_mut().clear();
        self.inner.deadlines.borrow_mut().clear();
        self.inner.damage.borrow_mut().clear();
        self.inner.test_ids.borrow_mut().clear();
        self.inner.copied_text.borrow_mut().take();
        self.inner.paste_requested.set(false);
        self.inner.cursor_icon.set(CursorIcon::Default);
        self.inner.accessibility.borrow_mut().clear();
    }

    pub fn end_frame(&self) -> FrameOutput {
        let shapes = Rc::new(std::mem::take(&mut *self.inner.shapes.borrow_mut()));
        let scale = self.pixels_per_point();
        let filter = self.inner.filter.take();
        let reported = std::mem::take(&mut *self.inner.damage.borrow_mut());
        let damage = reported
            .iter()
            .fold(Rect::NOTHING, |region, rect| region.union(*rect));
        let mut previous = self.inner.previous.borrow_mut();
        let changed = match previous.as_ref() {
            None => true,
            Some(old) if old.pixels_per_point != scale || old.filter != filter => true,
            Some(old) => match reported.is_empty() {
                true => {
                    debug_assert!(
                        *old.shapes == *shapes,
                        "a frame that reported no damage changed the shapes it painted"
                    );
                    false
                }
                false => *old.shapes != *shapes,
            },
        };
        if changed {
            *previous = Some(Previous {
                shapes: Rc::clone(&shapes),
                pixels_per_point: scale,
                filter,
            });
        }
        FrameOutput {
            shapes,
            filter,
            damage,
            test_ids: std::mem::take(&mut *self.inner.test_ids.borrow_mut()),
            copied_text: self.inner.copied_text.borrow_mut().take(),
            paste_requested: self.inner.paste_requested.replace(false),
            changed,
            repaint_after: self.inner.repaint_after.get(),
            cursor_icon: self.inner.cursor_icon.get(),
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

    pub fn touch_emulation(&self) -> bool {
        self.inner.touch_emulation.get()
    }

    pub(crate) fn set_touch_emulation(&self, enabled: bool) {
        self.inner.touch_emulation.set(enabled);
    }

    pub fn mouse_simulation(&self) -> bool {
        self.inner.mouse_simulation.borrow().enabled()
    }

    pub(crate) fn set_mouse_simulation(&self, enabled: bool) {
        self.inner
            .mouse_simulation
            .borrow_mut()
            .set_enabled(enabled);
    }

    pub(crate) fn measure_mouse_simulation(&self, viewport: Rect) -> f32 {
        let scale = self.native_pixels_per_point() / self.pixels_per_point();
        let simulation = &self.inner.mouse_simulation;
        simulation.borrow_mut().measure(viewport, scale);
        simulation.borrow().reserved()
    }

    pub(crate) fn show_mouse_simulation(&self, viewport: Rect) {
        let scale = self.native_pixels_per_point() / self.pixels_per_point();
        let simulation = &self.inner.mouse_simulation;
        if !simulation.borrow().painting() {
            return;
        }
        self.scaled(scale, || {
            let painter = self
                .painter()
                .with_clip_rect(viewport.scaled(scale.recip()));
            let damage = simulation.borrow_mut().paint(&painter);
            self.report_damage(damage);
        });
    }

    #[cfg(test)]
    pub(crate) fn simulated_cursor(&self) -> crate::geometry::Pos2 {
        self.inner.mouse_simulation.borrow().cursor()
    }

    #[cfg(test)]
    pub(crate) fn simulated_button(&self, index: usize) -> crate::geometry::Pos2 {
        self.inner.mouse_simulation.borrow().button_center(index)
    }

    #[cfg(test)]
    pub(crate) fn simulated_trackpad(&self) -> crate::geometry::Pos2 {
        self.inner.mouse_simulation.borrow().trackpad_center()
    }

    #[cfg(test)]
    pub(crate) fn simulated_key(&self, label: &str) -> Option<crate::geometry::Pos2> {
        self.inner.mouse_simulation.borrow().key_center(label)
    }

    pub fn request_repaint(&self) {
        self.inner.repaint.set(true);
        self.request_repaint_after(Duration::ZERO);
    }

    pub fn request_repaint_after(&self, delay: Duration) {
        self.inner
            .repaint_after
            .set(self.inner.repaint_after.get().min(delay));
        let deadline = self
            .inner
            .paint_stack
            .borrow()
            .last()
            .and_then(|frame| frame.id);
        if let Some(id) = deadline {
            self.inner.deadlines.borrow_mut().push(id);
        }
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn capture(&self, paint: impl FnOnce()) -> (Vec<Shape>, Duration) {
        let previous_delay = self.inner.repaint_after.replace(Duration::MAX);
        let start = self.inner.shapes.borrow().len();
        let base = self.inner.capture_base.replace(start);
        self.inner.deadlines.borrow_mut().clear();
        paint();
        self.inner.capture_base.set(base);
        let delay = self.inner.repaint_after.get();
        self.request_repaint_after(previous_delay);
        (self.inner.shapes.borrow_mut().split_off(start), delay)
    }

    pub(crate) fn measure_paint(&self, paint: impl FnOnce()) -> Rect {
        self.push_paint_frame(None);
        paint();
        self.exit_paint().bounds
    }

    pub(crate) fn enter_paint(&self, id: NodeId) {
        self.push_paint_frame(Some(id));
    }

    fn push_paint_frame(&self, id: Option<NodeId>) {
        let frame = PaintFrame {
            id,
            main_start: self.captured(),
            top_start: self.inner.top_shapes.borrow().len(),
            bounds: Rect::NOTHING,
            outer_delay: self.inner.repaint_after.replace(Duration::MAX),
        };
        self.inner.paint_stack.borrow_mut().push(frame);
    }

    pub(crate) fn exit_paint(&self) -> Painted {
        let frame = self
            .inner
            .paint_stack
            .borrow_mut()
            .pop()
            .expect("a painted node was entered");
        let top = self.inner.top_shapes.borrow()[frame.top_start..].to_vec();
        let delay = self.inner.repaint_after.get();
        self.inner.repaint_after.set(frame.outer_delay.min(delay));
        let painted = Painted {
            parent: self.parent_node(),
            main: frame.main_start..self.captured(),
            top,
            bounds: frame.bounds,
            deadline: (delay < Duration::MAX).then(|| Instant::now() + delay),
        };
        self.note_bounds(frame.bounds);
        painted
    }

    pub(crate) fn parent_start(&self) -> usize {
        self.inner
            .paint_stack
            .borrow()
            .last()
            .map_or(0, |frame| frame.main_start)
    }

    pub(crate) fn parent_node(&self) -> Option<NodeId> {
        self.inner
            .paint_stack
            .borrow()
            .last()
            .and_then(|frame| frame.id)
    }

    pub(crate) fn note_bounds(&self, bounds: Rect) {
        if let Some(frame) = self.inner.paint_stack.borrow_mut().last_mut() {
            frame.bounds = frame.bounds.union(bounds);
        }
    }

    pub(crate) fn take_deadlines(&self) -> Vec<NodeId> {
        let mut deadlines = std::mem::take(&mut *self.inner.deadlines.borrow_mut());
        deadlines.sort_unstable_by_key(|id| id.index());
        deadlines.dedup();
        deadlines
    }

    fn captured(&self) -> usize {
        self.inner.shapes.borrow().len() - self.inner.capture_base.get()
    }

    pub(crate) fn report_damage(&self, rect: Rect) {
        if rect.is_positive() {
            self.inner.damage.borrow_mut().push(rect);
        }
    }

    pub(crate) fn extend(&self, shapes: &[Shape]) {
        self.inner.shapes.borrow_mut().extend_from_slice(shapes);
    }

    pub(crate) fn extend_top(&self, shapes: &[Shape]) {
        self.inner.top_shapes.borrow_mut().extend_from_slice(shapes);
    }

    pub(crate) fn publish_test_id(&self, test_id: &str, rect: Rect) {
        if !self.inner.test_ids_published.get() {
            return;
        }
        self.inner
            .test_ids
            .borrow_mut()
            .insert(test_id.to_owned(), rect);
    }

    pub(crate) fn publish_accessibility(&self, fragment: Fragment) {
        self.inner.accessibility.borrow_mut().push(fragment);
    }

    pub fn accessibility_action(&self, request: ActionRequest) {
        self.inner.accessibility_actions.borrow_mut().push(request);
        self.request_repaint();
    }

    pub(crate) fn take_accessibility_actions(&self, document_id: u32) -> Vec<ActionRequest> {
        let mut actions = self.inner.accessibility_actions.borrow_mut();
        let all = std::mem::take(&mut *actions);
        let (matched, remaining) = all
            .into_iter()
            .partition(|request| request.target_node.0 >> 32 == document_id as u64);
        *actions = remaining;
        matched
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.inner.pixels_per_point.get()
    }

    pub fn set_pixels_per_point(&self, pixels_per_point: f32) {
        self.inner.native_pixels_per_point.set(pixels_per_point);
        self.apply_pixels_per_point();
    }

    pub(crate) fn native_pixels_per_point(&self) -> f32 {
        self.inner.native_pixels_per_point.get()
    }

    pub fn simulated_pixels_per_point(&self) -> Option<f32> {
        self.inner.simulated_pixels_per_point.get()
    }

    pub(crate) fn set_simulated_pixels_per_point(&self, pixels_per_point: Option<f32>) {
        self.inner.simulated_pixels_per_point.set(pixels_per_point);
    }

    fn apply_pixels_per_point(&self) {
        let pixels_per_point = self
            .simulated_pixels_per_point()
            .unwrap_or_else(|| self.native_pixels_per_point());
        self.inner.pixels_per_point.set(pixels_per_point);
    }

    pub(crate) fn scaled<R>(&self, scale: f32, content: impl FnOnce() -> R) -> R {
        if scale == 1.0 {
            return content();
        }
        let pixels_per_point = self.pixels_per_point();
        self.inner.pixels_per_point.set(pixels_per_point * scale);
        let input = self
            .inner
            .input
            .replace_with(|input| input.scaled(scale.recip()));
        let shapes = self.inner.shapes.borrow().len();
        let damage = self.inner.damage.borrow().len();
        let fragments = self.inner.accessibility.borrow().len();
        let test_ids = self.inner.test_ids.take();
        let result = content();
        self.inner.input.replace(input);
        self.inner.pixels_per_point.set(pixels_per_point);
        for shape in self.inner.shapes.borrow_mut().iter_mut().skip(shapes) {
            scale_shape(shape, scale);
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

    pub(crate) fn layout(&self, text: &str, font: FontId, layout: TextLayout) -> Galley {
        self.inner
            .fonts
            .borrow_mut()
            .layout(text, font, layout, self.pixels_per_point())
    }

    pub(crate) fn push(&self, shape: Shape) {
        self.note_shape(&shape);
        self.inner.shapes.borrow_mut().push(shape);
    }

    pub(crate) fn push_top(&self, shape: Shape) {
        self.note_shape(&shape);
        self.inner.top_shapes.borrow_mut().push(shape);
    }

    fn note_shape(&self, shape: &Shape) {
        let mut stack = self.inner.paint_stack.borrow_mut();
        if let Some(frame) = stack.last_mut() {
            frame.bounds = frame.bounds.union(damage::bounds(shape));
        }
    }

    pub(crate) fn apply_filter(&self, filter: Filter) {
        if filter.changes_nothing() || !filter.region.is_positive() {
            return;
        }
        let boundary = self.inner.shapes.borrow().len();
        self.inner.filter.set(Some((filter, boundary)));
    }

    pub(crate) fn flush_top(&self) {
        let top = std::mem::take(&mut *self.inner.top_shapes.borrow_mut());
        self.inner.shapes.borrow_mut().extend(top);
    }
}

struct Previous {
    shapes: Rc<Vec<Shape>>,
    pixels_per_point: f32,
    filter: Option<(Filter, usize)>,
}

struct PaintFrame {
    id: Option<NodeId>,
    main_start: usize,
    top_start: usize,
    bounds: Rect,
    outer_delay: Duration,
}

fn scale_shape(shape: &mut Shape, scale: f32) {
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

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
