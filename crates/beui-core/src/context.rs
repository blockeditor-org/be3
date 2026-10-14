use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{ActionRequest, TreeUpdate};

use crate::accessibility::{self, Fragment};
use crate::clipboard::{PasteKind, PasteRequest};
use crate::damage::{self, Region};
use crate::display::{Display, Group, Layer};
use crate::file_picker::{FileFilter, FilePick, FilePickId, FilePickRequest};
use crate::filter::Filter;
use crate::font::{FontBackend, FontId, Fonts, Galley, TextLayout};
use crate::geometry::Vec2;
use crate::geometry::{Pos2, Rect, pos2};
use crate::input::{CursorIcon, ImeArea, InputState, KeyChord, RawInput};
use crate::node::NodeId;
use crate::paint::{Item, Recorded, Recording};
use crate::painter::{Painter, Scope, Shape};
use crate::screen_simulation::ScreenSimulation;
use crate::screens::Screen;
use crate::window::WindowState;

type CopyWatcher = Rc<dyn Fn(&str)>;

#[derive(Clone)]
pub struct Context {
    inner: Rc<Inner>,
}

struct Inner {
    fonts: RefCell<Fonts>,
    input: RefCell<InputState>,
    layers: RefCell<Vec<Layer>>,
    grouping: RefCell<Vec<usize>>,
    paint_stack: RefCell<Vec<PaintFrame>>,
    space_read: Cell<bool>,
    verifying: Cell<bool>,
    damage: RefCell<Vec<Rect>>,
    moves: RefCell<Vec<(Moved, Vec<Rc<Display>>)>>,
    test_ids: RefCell<HashMap<String, Rect>>,
    ambiguous_test_ids: RefCell<HashSet<String>>,
    copied_text: RefCell<Option<String>>,
    paste: Cell<Option<PasteRequest>>,
    copy_watchers: RefCell<Vec<CopyWatcher>>,
    file_pick_requests: RefCell<Vec<FilePickRequest>>,
    file_picks: RefCell<Vec<(FilePickId, FilePick)>>,
    next_file_pick: Cell<u64>,
    window: RefCell<WindowState>,
    fullscreen: Cell<Option<bool>>,
    fullscreen_shown: Cell<bool>,
    close_requested: Cell<bool>,
    touch_emulation: Cell<bool>,
    input_simulation: RefCell<Option<Box<dyn InputSimulation>>>,
    simulation_area: Cell<(Rect, f32)>,
    mouse_viewport: Cell<Option<Rect>>,
    shown: Cell<Option<Rect>>,
    pixels_per_point: Cell<f32>,
    native_pixels_per_point: Cell<f32>,
    simulated_pixels_per_point: Cell<Option<f32>>,
    screen_simulation: Cell<Option<ScreenSimulation>>,
    screen_scale: Cell<f32>,
    screens: RefCell<Vec<Screen>>,
    zoom: Cell<f32>,
    repaint: Cell<bool>,
    repaint_after: Cell<Duration>,
    previous: RefCell<Option<Previous>>,
    accessibility: RefCell<Vec<Fragment>>,
    accessibility_actions: RefCell<Vec<ActionRequest>>,
    accessibility_active: Cell<bool>,
    automated: Cell<bool>,
    actions: RefCell<Vec<ActionGroup>>,
    action_requests: RefCell<Vec<String>>,
    hit_query: Cell<Option<Pos2>>,
    hit_answers: RefCell<Vec<String>>,
    accessibility_known: RefCell<HashSet<u32>>,
    accessibility_published: RefCell<HashSet<u32>>,
    test_ids_published: Cell<bool>,
    renderer_info: RefCell<Option<RendererInfo>>,
    renderers: RefCell<RendererChoices>,
    renderer_choice: Cell<Option<usize>>,
    clock: Cell<Option<Instant>>,
    now: Cell<Instant>,
}

pub trait InputSimulation: Any {
    fn enabled(&self) -> bool;

    fn set_enabled(&mut self, enabled: bool);

    fn translate(&mut self, raw: RawInput, now: Instant) -> (RawInput, Option<Duration>);

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

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RendererChoices {
    pub names: Vec<&'static str>,
    pub active: usize,
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

#[derive(Clone, Debug, PartialEq)]
pub struct ActionInfo {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub checked: Option<bool>,
    pub live: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionGroup {
    pub name: String,
    pub actions: Vec<ActionInfo>,
}

pub struct FrameOutput {
    pub layers: Rc<Vec<Layer>>,
    test_ids: HashMap<String, Rect>,
    ambiguous_test_ids: HashSet<String>,
    pub window: WindowState,
    pub fullscreen: Option<bool>,
    pub close_requested: bool,
    pub copied_text: Option<String>,
    pub paste: Option<PasteRequest>,
    pub file_picks: Vec<FilePickRequest>,
    pub repaint: bool,
    pub repaint_after: Duration,
    pub changed: bool,
    pub damage: Region,
    pub moved: Option<Moved>,
    accessibility: Vec<Fragment>,
    pub actions: Vec<ActionGroup>,
    pub unran_actions: Vec<String>,
    pub hits: Option<Vec<String>>,
    pixels_per_point: f32,
}

impl FrameOutput {
    pub fn shapes(&self) -> Vec<Shape> {
        crate::display::flatten(&self.layers)
    }

    pub fn filters(&self) -> Vec<Filter> {
        crate::display::filters(&self.layers)
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

    pub fn test_ids(&self) -> &HashMap<String, Rect> {
        &self.test_ids
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
        Self::with_fonts(Box::new(fonts))
    }

    pub fn with_fonts(fonts: Box<dyn FontBackend>) -> Self {
        Self {
            inner: Rc::new(Inner {
                fonts: RefCell::new(Fonts::boxed(fonts)),
                input: RefCell::new(InputState::default()),
                layers: RefCell::new(Vec::new()),
                grouping: RefCell::new(Vec::new()),
                paint_stack: RefCell::new(Vec::new()),
                space_read: Cell::new(false),
                verifying: Cell::new(false),
                damage: RefCell::new(Vec::new()),
                moves: RefCell::new(Vec::new()),
                test_ids: RefCell::new(HashMap::new()),
                ambiguous_test_ids: RefCell::new(HashSet::new()),
                copied_text: RefCell::new(None),
                paste: Cell::new(None),
                copy_watchers: RefCell::new(Vec::new()),
                file_pick_requests: RefCell::new(Vec::new()),
                file_picks: RefCell::new(Vec::new()),
                next_file_pick: Cell::new(0),
                window: RefCell::new(WindowState::default()),
                fullscreen: Cell::new(None),
                fullscreen_shown: Cell::new(false),
                close_requested: Cell::new(false),
                touch_emulation: Cell::new(false),
                input_simulation: RefCell::new(None),
                simulation_area: Cell::new((Rect::NOTHING, 1.0)),
                mouse_viewport: Cell::new(None),
                shown: Cell::new(None),
                pixels_per_point: Cell::new(1.0),
                native_pixels_per_point: Cell::new(1.0),
                simulated_pixels_per_point: Cell::new(None),
                screen_simulation: Cell::new(None),
                screen_scale: Cell::new(1.0),
                screens: RefCell::new(Vec::new()),
                zoom: Cell::new(1.0),
                repaint: Cell::new(false),
                repaint_after: Cell::new(Duration::MAX),
                previous: RefCell::new(None),
                accessibility: RefCell::new(Vec::new()),
                accessibility_actions: RefCell::new(Vec::new()),
                accessibility_active: Cell::new(true),
                automated: Cell::new(false),
                actions: RefCell::new(Vec::new()),
                action_requests: RefCell::new(Vec::new()),
                hit_query: Cell::new(None),
                hit_answers: RefCell::new(Vec::new()),
                accessibility_known: RefCell::new(HashSet::new()),
                accessibility_published: RefCell::new(HashSet::new()),
                test_ids_published: Cell::new(true),
                renderer_info: RefCell::new(None),
                renderers: RefCell::new(RendererChoices::default()),
                renderer_choice: Cell::new(None),
                clock: Cell::new(None),
                now: Cell::new(Instant::now()),
            }),
        }
    }

    pub fn set_renderer_info(&self, info: RendererInfo) {
        *self.inner.renderer_info.borrow_mut() = Some(info);
    }

    pub fn renderer_info(&self) -> Option<RendererInfo> {
        self.inner.renderer_info.borrow().clone()
    }

    pub fn set_renderers(&self, renderers: RendererChoices) {
        *self.inner.renderers.borrow_mut() = renderers;
    }

    pub fn renderers(&self) -> RendererChoices {
        self.inner.renderers.borrow().clone()
    }

    pub fn choose_renderer(&self, index: usize) {
        if index != self.inner.renderers.borrow().active {
            self.inner.renderer_choice.set(Some(index));
            self.request_repaint();
        }
    }

    pub fn take_renderer_choice(&self) -> Option<usize> {
        self.inner.renderer_choice.take()
    }

    pub fn replace_fonts(&self, fonts: Box<dyn FontBackend>) -> Box<dyn FontBackend> {
        self.inner.fonts.borrow_mut().replace(fonts)
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

    pub fn set_automated(&self, automated: bool) {
        self.inner.automated.set(automated);
    }

    pub fn automated(&self) -> bool {
        self.inner.automated.get()
    }

    pub fn publish_actions(&self, group: ActionGroup) {
        self.inner.actions.borrow_mut().push(group);
    }

    pub fn request_action(&self, id: String) {
        self.inner.action_requests.borrow_mut().push(id);
        self.request_repaint();
    }

    pub fn action_requests(&self) -> Vec<String> {
        self.inner.action_requests.borrow().clone()
    }

    pub fn request_hit(&self, pos: Pos2) {
        self.inner.hit_query.set(Some(pos));
        self.inner.hit_answers.borrow_mut().clear();
        self.request_repaint();
    }

    pub fn hit_query(&self) -> Option<Pos2> {
        self.inner.hit_query.get()
    }

    pub fn answer_hit(&self, answer: String) {
        self.inner.hit_answers.borrow_mut().push(answer);
    }

    pub fn action_ran(&self, id: &str) {
        self.inner
            .action_requests
            .borrow_mut()
            .retain(|requested| requested != id);
    }

    pub fn set_test_ids_published(&self, published: bool) {
        self.inner.test_ids_published.set(published);
    }

    pub fn test_ids_published(&self) -> bool {
        self.inner.test_ids_published.get()
    }

    pub fn now(&self) -> Instant {
        self.inner.now.get()
    }

    pub fn stop_clock(&self) {
        self.inner.clock.set(Some(self.inner.now.get()));
    }

    pub fn run_clock(&self) {
        self.inner.clock.set(None);
    }

    pub fn set_clock(&self, now: Instant) {
        self.inner.clock.set(Some(now));
        self.inner.now.set(now);
    }

    pub fn advance_clock(&self, by: Duration) {
        let now = self
            .inner
            .clock
            .get()
            .unwrap_or_else(|| self.inner.now.get())
            + by;
        self.inner.clock.set(Some(now));
        self.inner.now.set(now);
    }

    pub fn begin_frame(&self, raw: RawInput) {
        let now = self.inner.clock.get().unwrap_or_else(Instant::now);
        self.inner.now.set(now);
        self.apply_pixels_per_point();
        self.inner.repaint.set(false);
        self.inner.repaint_after.set(Duration::MAX);
        self.inner.mouse_viewport.set(None);
        self.inner.shown.set(None);
        let (raw, wake) = match self.inner.input_simulation.borrow_mut().as_mut() {
            Some(simulation) => simulation.translate(raw, now),
            None => (raw, None),
        };
        if let Some(delay) = wake {
            self.request_repaint_after(delay);
        }
        self.inner.input.borrow_mut().begin_frame(raw, now);
        self.inner.layers.borrow_mut().clear();
        self.inner.grouping.borrow_mut().clear();
        self.inner.paint_stack.borrow_mut().clear();
        self.inner.damage.borrow_mut().clear();
        self.inner.moves.borrow_mut().clear();
        self.inner.test_ids.borrow_mut().clear();
        self.inner.ambiguous_test_ids.borrow_mut().clear();
        self.inner.copied_text.borrow_mut().take();
        self.inner.paste.set(None);
        self.inner.window.borrow_mut().begin_frame();
        self.inner.fullscreen.set(None);
        self.inner.close_requested.set(false);
        self.inner.accessibility.borrow_mut().clear();
        self.inner.actions.borrow_mut().clear();
        let published = std::mem::take(&mut *self.inner.accessibility_published.borrow_mut());
        *self.inner.accessibility_known.borrow_mut() = published;
    }

    pub fn end_frame(&self) -> FrameOutput {
        if let Some(viewport) = self.inner.mouse_viewport.take() {
            self.paint_mouse_simulation(viewport);
        }
        let layers = Rc::new(std::mem::take(&mut *self.inner.layers.borrow_mut()));
        let scale = self.pixels_per_point();
        let filters = crate::display::filters(&layers);
        let moved = self.settle_moves(&layers, scale);
        let previous = self.inner.previous.replace(Some(Previous {
            pixels_per_point: scale,
            filters: filters.clone(),
        }));
        if let Some(old) = &previous {
            for index in 0..old.filters.len().max(filters.len()) {
                let (was, now) = (old.filters.get(index), filters.get(index));
                if was != now {
                    for filter in [was, now].into_iter().flatten() {
                        self.report_damage(filter.region);
                    }
                }
            }
        }
        let reported = std::mem::take(&mut *self.inner.damage.borrow_mut());
        let damage = reported
            .iter()
            .fold(Region::NOTHING, |region, rect| region.union((*rect).into()));
        let changed = match previous {
            Some(old) if old.pixels_per_point == scale => !damage.is_empty() || moved.is_some(),
            _ => true,
        };
        let copied_text = self.inner.copied_text.borrow_mut().take();
        if let Some(text) = &copied_text {
            let watchers = self.inner.copy_watchers.borrow().clone();
            for watcher in watchers {
                watcher(text);
            }
        }
        FrameOutput {
            layers,
            damage,
            moved,
            test_ids: std::mem::take(&mut *self.inner.test_ids.borrow_mut()),
            ambiguous_test_ids: std::mem::take(&mut *self.inner.ambiguous_test_ids.borrow_mut()),
            copied_text,
            paste: self.inner.paste.take(),
            file_picks: std::mem::take(&mut *self.inner.file_pick_requests.borrow_mut()),
            changed,
            repaint_after: self.inner.repaint_after.get(),
            window: WindowState {
                touch_emulation: self.touch_emulation(),
                ..self.inner.window.borrow().clone()
            },
            fullscreen: self.inner.fullscreen.get(),
            close_requested: self.inner.close_requested.get(),
            repaint: self.inner.repaint.get(),
            accessibility: std::mem::take(&mut *self.inner.accessibility.borrow_mut()),
            actions: std::mem::take(&mut *self.inner.actions.borrow_mut()),
            unran_actions: std::mem::take(&mut *self.inner.action_requests.borrow_mut()),
            hits: self
                .inner
                .hit_query
                .take()
                .map(|_| std::mem::take(&mut *self.inner.hit_answers.borrow_mut())),
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

    pub fn set_ime_area(&self, area: Option<ImeArea>) {
        self.inner.window.borrow_mut().ime = area;
    }

    pub fn ime_area(&self) -> Option<ImeArea> {
        self.inner.window.borrow().ime.clone()
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.inner.fullscreen.set(Some(fullscreen));
    }

    pub fn fullscreen(&self) -> bool {
        self.inner.fullscreen_shown.get()
    }

    pub fn fullscreen_changed(&self, fullscreen: bool) {
        if self.inner.fullscreen_shown.replace(fullscreen) != fullscreen {
            self.request_repaint();
        }
    }

    pub fn close_window(&self) {
        self.inner.close_requested.set(true);
    }

    pub fn handle_back(&self) {
        self.inner.window.borrow_mut().handles_back = true;
    }

    pub fn want_keyboard(&self) {
        self.inner.window.borrow_mut().wants_keyboard = true;
    }

    pub fn intercept_keys(&self, chords: &[KeyChord]) {
        let mut window = self.inner.window.borrow_mut();
        for chord in chords {
            if !window.intercepted_keys.contains(chord) {
                window.intercepted_keys.push(*chord);
            }
        }
    }

    pub fn painter(&self) -> Painter {
        Painter::new(self.clone(), Rect::EVERYTHING)
    }

    pub fn copy_text(&self, text: String) {
        *self.inner.copied_text.borrow_mut() = Some(text);
    }

    pub fn request_paste(&self) {
        if self.inner.paste.get().is_none() {
            self.inner.paste.set(Some(PasteRequest::Asked));
        }
    }

    pub fn paste_shortcut(&self) {
        self.inner.paste.set(Some(PasteRequest::Shortcut));
    }

    pub fn accept_pastes(&self, kinds: &[PasteKind]) {
        self.inner.window.borrow_mut().paste_kinds = kinds.to_vec();
    }

    pub fn watch_copies(&self, watcher: impl Fn(&str) + 'static) {
        self.inner.copy_watchers.borrow_mut().push(Rc::new(watcher));
    }

    pub fn pick_file(&self, filter: FileFilter) -> FilePickId {
        let id = FilePickId(self.inner.next_file_pick.get() + 1);
        self.inner.next_file_pick.set(id.0);
        self.inner
            .file_pick_requests
            .borrow_mut()
            .push(FilePickRequest { id, filter });
        id
    }

    pub fn file_picked(&self, id: FilePickId, pick: FilePick) {
        self.inner.file_picks.borrow_mut().push((id, pick));
    }

    pub fn take_file_pick(&self, id: FilePickId) -> Option<FilePick> {
        let mut picks = self.inner.file_picks.borrow_mut();
        let index = picks.iter().position(|(picked, _)| *picked == id)?;
        Some(picks.remove(index).1)
    }

    pub fn set_pointer_locked(&self, locked: bool) {
        let changed =
            std::mem::replace(&mut self.inner.window.borrow_mut().pointer_locked, locked) != locked;
        if changed {
            self.request_repaint();
        }
    }

    pub fn pointer_locked(&self) -> bool {
        self.inner.window.borrow().pointer_locked
    }

    pub fn set_cursor_icon(&self, cursor_icon: CursorIcon) {
        self.inner.window.borrow_mut().cursor = cursor_icon;
    }

    pub fn cursor_icon(&self) -> CursorIcon {
        self.inner.window.borrow().cursor
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

    pub fn verifying_paint(&self, verifying: bool) {
        self.inner.verifying.set(verifying);
    }

    pub fn is_verifying_paint(&self) -> bool {
        self.inner.verifying.get()
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
            deadline: (frame.delay < Duration::MAX).then(|| self.now() + frame.delay),
            reads: false,
        }
    }

    pub fn paint_child(&self, scope: &Option<Rc<Scope>>, clip: Rect, id: NodeId, at: Vec2) {
        if let Some(frame) = self.inner.paint_stack.borrow_mut().last_mut()
            && frame.id.is_some()
        {
            frame.items.push(Item {
                scope: scope.clone(),
                clip,
                recording: Recording::Child(id, at),
            });
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
                    && !overlaps_besides(layers, roots, moved.from.union(moved.to())) =>
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

    pub fn show_painting(&self, painting: &[(Rc<Display>, Vec2)], within: Rect) {
        let shown = self.inner.shown.get();
        self.inner
            .shown
            .set(Some(shown.map_or(within, |shown| shown.union(within))));
        self.inner.layers.borrow_mut().extend(
            painting
                .iter()
                .map(|(display, at)| Layer::Display(*at, Rc::clone(display))),
        );
    }

    fn grouping<R>(&self, layers: usize, content: impl FnOnce() -> R) -> R {
        self.inner.grouping.borrow_mut().push(layers);
        let result = content();
        self.inner.grouping.borrow_mut().pop();
        result
    }

    fn group_since(&self, layers: usize, group: Group) {
        let mut held = self.inner.layers.borrow_mut();
        let inner: Rc<[Layer]> = held.drain(layers..).collect();
        if !inner.is_empty() {
            held.push(Layer::Group(group, inner));
        }
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

    pub fn screens(&self) -> Vec<Screen> {
        self.inner.screens.borrow().clone()
    }

    pub fn set_screens(&self, screens: Vec<Screen>) {
        if *self.inner.screens.borrow() != screens {
            *self.inner.screens.borrow_mut() = screens;
            self.request_repaint();
        }
    }

    pub fn set_screen_simulation(&self, simulation: Option<ScreenSimulation>) {
        self.inner.screen_simulation.set(simulation);
    }

    pub fn clipped<R>(&self, clip: Rect, content: impl FnOnce() -> R) -> R {
        let layers = self.inner.layers.borrow().len();
        let damage = self.inner.damage.borrow().len();
        let moves = self.inner.moves.borrow().len();
        let result = self.grouping(layers, content);
        self.unmove(moves);
        self.group_since(layers, Group::Clip(clip));
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

    pub fn with_input<R>(
        &self,
        input: impl FnOnce(&InputState) -> InputState,
        content: impl FnOnce() -> R,
    ) -> R {
        let given = input(&self.inner.input.borrow());
        let held = self.inner.input.replace(given);
        let result = content();
        self.inner.input.replace(held);
        result
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
        let ime = self.inner.window.borrow_mut().ime.take();
        let result = self.grouping(layers, content);
        self.unmove(moves);
        let scaled_ime = std::mem::replace(&mut self.inner.window.borrow_mut().ime, ime);
        if let Some(area) = scaled_ime {
            self.inner.window.borrow_mut().ime = Some(ImeArea {
                rect: area.rect.scaled(scale),
                cursor: area.cursor.scaled(scale),
                ..area
            });
        }
        self.inner.input.replace(input);
        self.inner.pixels_per_point.set(pixels_per_point);
        self.group_since(
            layers,
            Group::Transform {
                offset: Vec2::ZERO,
                scale,
            },
        );
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

    pub fn font_generation(&self) -> u64 {
        self.inner.fonts.borrow_mut().generation()
    }

    pub fn layout(&self, text: &str, font: FontId, layout: TextLayout) -> Galley {
        self.inner
            .fonts
            .borrow_mut()
            .layout(text, font, layout, self.pixels_per_point())
    }

    pub fn push(&self, shape: Shape) {
        self.push_in(&None, shape);
    }

    pub fn push_in(&self, scope: &Option<Rc<Scope>>, shape: Shape) {
        let mut stack = self.inner.paint_stack.borrow_mut();
        if let Some(frame) = stack.last_mut()
            && frame.id.is_some()
        {
            if damage::bounds(&shape).is_positive() || matches!(shape, Shape::Drawing { .. }) {
                frame.items.push(Item {
                    scope: scope.clone(),
                    clip: Rect::EVERYTHING,
                    recording: Recording::Shape(shape),
                });
            }
            return;
        }
        let mut shape = shape;
        for scope in Scope::chain(scope).into_iter().rev() {
            scope.group.place_shape(&mut shape);
        }
        if let Some(frame) = stack.last_mut() {
            frame.bounds = frame.bounds.union(damage::bounds(&shape));
        }
        drop(stack);
        self.inner.layers.borrow_mut().push(Layer::Shape(shape));
    }

    pub fn apply_filter(&self, filter: Filter) {
        if filter.changes_nothing() || !filter.region.is_positive() {
            return;
        }
        let from = self.inner.grouping.borrow().last().copied().unwrap_or(0);
        self.group_since(from, Group::Filter(filter));
    }
}

fn overlaps_besides(layers: &[Layer], roots: &[Rc<Display>], area: Rect) -> bool {
    layers.iter().any(|layer| match layer {
        Layer::Shape(shape) => damage::bounds(shape).intersects(area),
        Layer::Display(at, display) => {
            !roots.iter().any(|root| Rc::ptr_eq(root, display))
                && display.bounds.translate(*at).intersects(area)
        }
        Layer::Group(Group::Filter(filter), inner) => {
            filter.region.intersects(area) || overlaps_besides(inner, roots, area)
        }
        Layer::Group(group, inner) => {
            let within = match *group {
                Group::Transform { offset, scale } if scale > 0.0 => {
                    area.translate(-offset).scaled(scale.recip())
                }
                Group::Transform { .. } => return true,
                Group::Clip(clip) => area.intersect(clip),
                Group::Scroll {
                    viewport, offset, ..
                } => area.intersect(viewport).translate(-offset),
                Group::Mask { .. } | Group::Filter(_) => area,
            };
            within.is_positive() && overlaps_besides(inner, roots, within)
        }
    })
}

struct Previous {
    pixels_per_point: f32,
    filters: Vec<Filter>,
}

struct PaintFrame {
    id: Option<NodeId>,
    items: Vec<Item>,
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
            *corner_radius = corner_radius.scaled(scale);
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
