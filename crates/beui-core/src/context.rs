use foldhash::HashSet;
use std::any::Any;
use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{ActionRequest, TreeUpdate};

use crate::accessibility::{self, Fragment};
use crate::app::Waker;
use crate::app::accessibility_dump::{AccessibilityDump, TestIds};
use crate::clipboard::{PasteKind, PasteRequest};
use crate::damage::{self, Region};
use crate::display::{Display, Group, Layer};
use crate::file_picker::{FileFilter, FilePick, FilePickId, FilePickRequest};
use crate::file_saver::{FileSave, FileSaveId, FileSaveRequest, SavedFile};
use crate::filter::Filter;
use crate::font::{FontBackend, FontId, Fonts, Galley, TextLayout};
use crate::geometry::Vec2;
use crate::geometry::{Pos2, Rect, pos2};
use crate::input::{CursorIcon, Event, ImeArea, InputState, KeyChord};
use crate::node::NodeId;
use crate::paint::{Item, Recorded, Recording};
use crate::painter::{Painter, PainterState, Scope, Shape};
use crate::preferences::{PreferenceLayer, PreferenceLayers, PreferenceSource, Preferences};
use crate::screen_simulation::ScreenSimulation;
use crate::screens::Screen;
use crate::window::{
    Appearance, HostPlacement, WindowHosting, WindowId, WindowReport, WindowRequest, WindowState,
};

type CopyWatcher = Rc<dyn Fn(&str)>;

#[derive(Default)]
struct Kept {
    dump: AccessibilityDump,
    viewport: Option<Vec2>,
}

#[derive(Clone, Default)]
struct KnownWindow {
    size: Option<Vec2>,
    placement: Option<HostPlacement>,
    pixels_per_point: Option<f32>,
}

struct ShownWindow {
    id: WindowId,
    size: Vec2,
    layers: Vec<Layer>,
    damage: Region,
    moved: Option<Moved>,
    title: Option<String>,
    appearance: Option<Appearance>,
}

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
    copied_text: RefCell<Option<String>>,
    copied_image: RefCell<Option<Vec<u8>>>,
    paste: Cell<Option<PasteRequest>>,
    copy_watchers: RefCell<Vec<CopyWatcher>>,
    file_pick_requests: RefCell<Vec<FilePickRequest>>,
    file_picks: RefCell<Vec<(FilePickId, FilePick)>>,
    next_file_pick: Cell<u64>,
    file_save_requests: RefCell<Vec<FileSaveRequest>>,
    file_saves: RefCell<Vec<(FileSaveId, FileSave)>>,
    next_file_save: Cell<u64>,
    window: RefCell<WindowState>,
    window_requests: RefCell<Vec<WindowRequest>>,
    windows: RefCell<foldhash::HashMap<WindowId, KnownWindow>>,
    window_reports: RefCell<Vec<WindowReport>>,
    shown_windows: RefCell<Vec<ShownWindow>>,
    keyboard_window: Cell<WindowId>,
    window_hosting: Cell<WindowHosting>,
    shows_main: Cell<bool>,
    fullscreen: Cell<Option<bool>>,
    fullscreen_shown: Cell<bool>,
    close_requested: Cell<bool>,
    touch_emulation: Cell<bool>,
    preferences: Cell<PreferenceLayers>,
    input_simulation: RefCell<Option<Box<dyn InputSimulation>>>,
    keyboard: RefCell<crate::software_keyboard::KeyboardState>,
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
    animating: Cell<bool>,
    previous: RefCell<Option<Previous>>,
    accessibility: RefCell<Vec<Fragment>>,
    accessibility_actions: RefCell<Vec<ActionRequest>>,
    accessibility_active: Cell<bool>,
    accessibility_requested: Cell<bool>,
    automated: Cell<bool>,
    unhurried: Cell<bool>,
    waker: RefCell<Option<Waker>>,
    in_frame: Cell<bool>,
    woken: Cell<bool>,
    zones_woken_in_frame: RefCell<Vec<u64>>,
    seen: Cell<bool>,
    actions: RefCell<Vec<ActionGroup>>,
    action_requests: RefCell<Vec<(Option<WindowId>, String)>>,
    hit_query: Cell<Option<Pos2>>,
    hit_answers: RefCell<Vec<String>>,
    comps_query: Cell<Option<bool>>,
    comps_answers: RefCell<Vec<String>>,
    overflows: RefCell<Vec<(WindowId, String)>>,
    accessibility_known: RefCell<HashSet<u32>>,
    accessibility_published: RefCell<HashSet<u32>>,
    kept: RefCell<Option<Kept>>,
    renderer_info: RefCell<Option<RendererInfo>>,
    renderers: RefCell<RendererChoices>,
    renderer_choice: Cell<Option<usize>>,
    clock: Cell<Option<Instant>>,
    now: Cell<Instant>,
}

pub trait InputSimulation: Any {
    fn enabled(&self) -> bool;

    fn set_enabled(&mut self, enabled: bool);

    fn translate(&mut self, events: Vec<Event>, now: Instant) -> (Vec<Event>, Option<Duration>);

    fn measure(&mut self, viewport: Rect, scale: f32);

    fn reserved(&self) -> f32;

    fn keyboard(&mut self, _shown: Option<Rect>) {}

    fn take_keyboard_toggle(&mut self) -> bool {
        false
    }

    fn painting(&self) -> bool;

    fn paint(&mut self, painter: &Painter, icon: CursorIcon) -> Rect;

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl Inner {
    fn wake(&self) {
        if self.woken.replace(true) {
            return;
        }
        let waker = self.waker.borrow().clone();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
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
    pub window: Option<WindowId>,
    pub actions: Vec<ActionInfo>,
}

#[derive(Clone)]
pub struct WindowOutput {
    pub id: WindowId,
    pub size: Vec2,
    pub layers: Rc<Vec<Layer>>,
    pub window: WindowState,
    pub changed: bool,
    pub damage: Region,
    pub moved: Option<Moved>,
    pub pixels_per_point: f32,
}

impl WindowOutput {
    pub fn shapes(&self) -> Vec<Shape> {
        crate::display::flatten(&self.layers)
    }

    pub fn filters(&self) -> Vec<Filter> {
        crate::display::filters(&self.layers)
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
}

pub struct FrameOutput {
    pub windows: Vec<WindowOutput>,
    pub window_requests: Vec<WindowRequest>,
    dump: Option<AccessibilityDump>,
    pub fullscreen: Option<bool>,
    pub close_requested: bool,
    pub copied_text: Option<String>,
    pub copied_image: Option<Vec<u8>>,
    pub paste: Option<PasteRequest>,
    pub file_picks: Vec<FilePickRequest>,
    pub file_saves: Vec<FileSaveRequest>,
    pub repaint: bool,
    pub repaint_after: Duration,
    pub animating: bool,
    accessibility: Vec<Fragment>,
    pub actions: Vec<ActionGroup>,
    pub unran_actions: Vec<String>,
    pub hits: Option<Vec<String>>,
    pub comps: Option<Vec<String>>,
    pub overflows: Vec<String>,
    pub window_overflows: Vec<(WindowId, String)>,
    pub typed: Vec<crate::input::Input>,
    pixels_per_point: f32,
}

impl FrameOutput {
    pub fn main(&self) -> &WindowOutput {
        self.window(WindowId::MAIN)
            .expect("a frame always holds the main window")
    }

    pub fn main_mut(&mut self) -> &mut WindowOutput {
        self.windows
            .iter_mut()
            .find(|window| window.id == WindowId::MAIN)
            .expect("a frame always holds the main window")
    }

    pub fn window(&self, id: WindowId) -> Option<&WindowOutput> {
        self.windows.iter().find(|window| window.id == id)
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    pub fn test_ids(&self) -> Rc<TestIds> {
        self.dump
            .as_ref()
            .map(|dump| Rc::clone(dump.rendered().test_ids()))
            .unwrap_or_default()
    }

    pub fn test_id_rect(&self, test_id: &str) -> Option<Rect> {
        self.test_ids().rect(test_id)
    }

    pub fn accessibility_tree(&self, title: &str, viewport: crate::geometry::Vec2) -> TreeUpdate {
        accessibility::tree_update(
            title,
            viewport,
            self.pixels_per_point,
            self.accessibility.clone(),
        )
    }

    pub fn window_accessibility_tree(
        &self,
        window: WindowId,
        title: &str,
        viewport: crate::geometry::Vec2,
    ) -> TreeUpdate {
        accessibility::window_tree_update(
            window,
            title,
            viewport,
            self.pixels_per_point,
            &self.accessibility,
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
                copied_text: RefCell::new(None),
                copied_image: RefCell::new(None),
                paste: Cell::new(None),
                copy_watchers: RefCell::new(Vec::new()),
                file_pick_requests: RefCell::new(Vec::new()),
                file_picks: RefCell::new(Vec::new()),
                next_file_pick: Cell::new(0),
                file_save_requests: RefCell::new(Vec::new()),
                file_saves: RefCell::new(Vec::new()),
                next_file_save: Cell::new(0),
                window: RefCell::new(WindowState::default()),
                window_requests: RefCell::new(Vec::new()),
                windows: RefCell::new(foldhash::HashMap::default()),
                window_reports: RefCell::new(Vec::new()),
                shown_windows: RefCell::new(Vec::new()),
                keyboard_window: Cell::new(WindowId::MAIN),
                window_hosting: Cell::new(WindowHosting::None),
                shows_main: Cell::new(true),
                fullscreen: Cell::new(None),
                fullscreen_shown: Cell::new(false),
                close_requested: Cell::new(false),
                touch_emulation: Cell::new(false),
                preferences: Cell::new(PreferenceLayers::default()),
                input_simulation: RefCell::new(None),
                keyboard: RefCell::new(Default::default()),
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
                animating: Cell::new(false),
                previous: RefCell::new(None),
                accessibility: RefCell::new(Vec::new()),
                accessibility_actions: RefCell::new(Vec::new()),
                accessibility_active: Cell::new(true),
                accessibility_requested: Cell::new(false),
                automated: Cell::new(false),
                unhurried: Cell::new(false),
                waker: RefCell::new(None),
                in_frame: Cell::new(false),
                woken: Cell::new(false),
                zones_woken_in_frame: RefCell::new(Vec::new()),
                seen: Cell::new(true),
                actions: RefCell::new(Vec::new()),
                action_requests: RefCell::new(Vec::new()),
                hit_query: Cell::new(None),
                hit_answers: RefCell::new(Vec::new()),
                comps_query: Cell::new(None),
                comps_answers: RefCell::new(Vec::new()),
                overflows: RefCell::new(Vec::new()),
                accessibility_known: RefCell::new(HashSet::default()),
                accessibility_published: RefCell::new(HashSet::default()),
                kept: RefCell::new(Some(Kept::default())),
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

    pub fn set_accessibility_requested(&self, requested: bool) {
        if self.inner.accessibility_requested.replace(requested) != requested {
            self.request_repaint();
        }
    }

    pub fn accessibility_requested(&self) -> bool {
        self.inner.accessibility_requested.get()
    }

    pub fn set_automated(&self, automated: bool) {
        self.inner.automated.set(automated);
    }

    pub fn automated(&self) -> bool {
        self.inner.automated.get()
    }

    pub fn set_hurried(&self, hurried: bool) {
        self.inner.unhurried.set(!hurried);
    }

    pub fn hurried(&self) -> bool {
        self.automated() && !self.inner.unhurried.get()
    }

    pub fn set_seen(&self, seen: bool) {
        self.inner.seen.set(seen);
    }

    pub fn seen(&self) -> bool {
        self.inner.seen.get()
    }

    pub fn publish_actions(&self, group: ActionGroup) {
        self.inner.actions.borrow_mut().push(group);
    }

    pub fn request_action(&self, id: String) {
        self.inner.action_requests.borrow_mut().push((None, id));
        self.request_repaint();
    }

    pub fn request_action_in(&self, window: WindowId, id: String) {
        self.inner
            .action_requests
            .borrow_mut()
            .push((Some(window), id));
        self.request_repaint();
    }

    pub fn action_requests(&self) -> Vec<(Option<WindowId>, String)> {
        self.inner.action_requests.borrow().clone()
    }

    pub fn request_comps(&self, internals: bool) {
        self.inner.comps_query.set(Some(internals));
        self.inner.comps_answers.borrow_mut().clear();
        self.request_repaint();
    }

    pub fn comps_query(&self) -> Option<bool> {
        self.inner.comps_query.get()
    }

    pub fn answer_comps(&self, answer: String) {
        self.inner.comps_answers.borrow_mut().push(answer);
    }

    pub fn report_overflows(&self, found: &[(WindowId, String)]) {
        self.inner.overflows.borrow_mut().extend_from_slice(found);
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
            .retain(|(_, requested)| requested != id);
    }

    pub fn keep_accessibility(&self, keep: bool) {
        let mut kept = self.inner.kept.borrow_mut();
        if keep != kept.is_some() {
            *kept = keep.then(Kept::default);
            drop(kept);
            self.reset_accessibility();
        }
    }

    pub fn set_accessibility_viewport(&self, viewport: Vec2) {
        if let Some(kept) = &mut *self.inner.kept.borrow_mut() {
            kept.viewport = Some(viewport);
        }
    }

    pub fn kept_accessibility(&self) -> Option<Ref<'_, AccessibilityDump>> {
        Ref::filter_map(self.inner.kept.borrow(), |kept| {
            kept.as_ref().map(|kept| &kept.dump)
        })
        .ok()
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

    pub fn clock_stopped(&self) -> bool {
        self.inner.clock.get().is_some()
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

    pub(crate) fn frame_time(&self) -> Instant {
        self.inner.clock.get().unwrap_or_else(Instant::now)
    }

    fn begin_frame(&self, events: Vec<Event>, now: Instant) {
        self.inner.in_frame.set(true);
        self.inner.woken.set(false);
        self.inner.now.set(now);
        self.apply_pixels_per_point();
        self.inner.repaint.set(false);
        self.inner.repaint_after.set(Duration::MAX);
        self.inner.animating.set(false);
        self.inner.mouse_viewport.set(None);
        self.inner.shown.set(None);
        let keyboard = self.keyboard_rect();
        let (events, wake, toggled) = match self.inner.input_simulation.borrow_mut().as_mut() {
            Some(simulation) => {
                simulation.keyboard(keyboard);
                let (events, wake) = simulation.translate(events, now);
                (events, wake, simulation.take_keyboard_toggle())
            }
            None => (events, None, false),
        };
        if toggled {
            self.toggle_keyboard();
        }
        if let Some(delay) = wake {
            self.request_repaint_after(delay);
        }
        let typed = events
            .iter()
            .any(|event| matches!(event, Event::Ime(_) | Event::Text(_)));
        self.inner.window.borrow_mut().begin_frame(typed);
        self.inner.input.borrow_mut().begin_frame(events, now);
        self.inner.layers.borrow_mut().clear();
        self.inner.grouping.borrow_mut().clear();
        self.inner.paint_stack.borrow_mut().clear();
        self.inner.damage.borrow_mut().clear();
        self.inner.moves.borrow_mut().clear();
        self.inner.copied_text.borrow_mut().take();
        self.inner.copied_image.borrow_mut().take();
        self.inner.paste.set(None);
        self.inner.fullscreen.set(None);
        self.inner.window_requests.borrow_mut().clear();
        self.inner.shown_windows.borrow_mut().clear();
        self.inner.keyboard_window.set(WindowId::MAIN);
        self.inner.close_requested.set(false);
        self.inner.accessibility.borrow_mut().clear();
        self.inner.actions.borrow_mut().clear();
        self.inner.overflows.borrow_mut().clear();
        let published = std::mem::take(&mut *self.inner.accessibility_published.borrow_mut());
        *self.inner.accessibility_known.borrow_mut() = published;
        ::reactive::flush();
    }

    fn end_frame(&self) -> FrameOutput {
        ::reactive::flush();
        self.inner.in_frame.set(false);
        let woken = std::mem::take(&mut *self.inner.zones_woken_in_frame.borrow_mut());
        if woken.into_iter().any(::reactive::zone_pending) {
            self.request_repaint();
        }
        if let Some(viewport) = self.inner.mouse_viewport.take() {
            self.paint_mouse_simulation(viewport);
        }
        let layers = Rc::new(std::mem::take(&mut *self.inner.layers.borrow_mut()));
        let scale = self.pixels_per_point();
        let filters = crate::display::filters(&layers);
        self.inner.window_reports.borrow_mut().clear();
        let shown_ids: Vec<WindowId> = self
            .inner
            .shown_windows
            .borrow()
            .iter()
            .map(|shown| shown.id)
            .collect();
        let window_scales: Vec<(WindowId, f32)> = std::iter::once(WindowId::MAIN)
            .chain(shown_ids)
            .map(|window| (window, self.window_pixels_per_point(window)))
            .collect();
        let previous = self.inner.previous.replace(Some(Previous {
            pixels_per_point: scale,
            windows: window_scales.clone(),
            filters: filters.clone(),
        }));
        let rescaled = |window: WindowId, now: f32| {
            previous.as_ref().is_none_or(|old| {
                old.pixels_per_point != scale
                    || old
                        .windows
                        .iter()
                        .find(|(held, _)| *held == window)
                        .is_some_and(|(_, was)| *was != now)
            })
        };
        let main_scale = window_scales[0].1;
        let main_rescaled = rescaled(WindowId::MAIN, main_scale);
        let main_resampled = main_rescaled
            && previous
                .as_ref()
                .is_some_and(|old| old.pixels_per_point == scale);
        let moved = self
            .settle_moves(&layers, main_scale)
            .filter(|_| !main_resampled);
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
        let changed = main_rescaled || !damage.is_empty() || moved.is_some();
        let copied_text = self.inner.copied_text.borrow_mut().take();
        if let Some(text) = &copied_text {
            let watchers = self.inner.copy_watchers.borrow().clone();
            for watcher in watchers {
                watcher(text);
            }
        }
        let accessibility = std::mem::take(&mut *self.inner.accessibility.borrow_mut());
        let dump = match &mut *self.inner.kept.borrow_mut() {
            Some(kept) => {
                let window = self.inner.window.borrow();
                kept.dump.update(accessibility::tree_update(
                    window.appearance.title.as_deref().unwrap_or_default(),
                    kept.viewport.unwrap_or(Vec2::splat(f32::MAX)),
                    scale,
                    accessibility.clone(),
                ));
                Some(kept.dump.clone())
            }
            None => None,
        };
        let (main, others) = self.window_states(self.shown_window());
        let main_size = self.window_size(WindowId::MAIN).unwrap_or(Vec2::ZERO);
        let mut windows = vec![WindowOutput {
            id: WindowId::MAIN,
            size: main_size,
            layers,
            window: main,
            changed,
            damage: match main_resampled {
                true => Region::from(Rect::from_min_size(Pos2::ZERO, main_size)),
                false => damage,
            },
            moved,
            pixels_per_point: main_scale,
        }];
        let shown = std::mem::take(&mut *self.inner.shown_windows.borrow_mut());
        for ((shown, window), (_, window_scale)) in shown
            .into_iter()
            .zip(others)
            .zip(window_scales.into_iter().skip(1))
        {
            let (damage, moved) = match rescaled(shown.id, window_scale) {
                true => (
                    Region::from(Rect::from_min_size(Pos2::ZERO, shown.size)),
                    None,
                ),
                false => match shown.moved {
                    Some(moved) if whole_pixels(moved.by, window_scale) => {
                        (shown.damage, Some(moved))
                    }
                    Some(moved) => (shown.damage.union(moved.to().into()), None),
                    None => (shown.damage, None),
                },
            };
            windows.push(WindowOutput {
                id: shown.id,
                size: shown.size,
                layers: Rc::new(shown.layers),
                window,
                changed: !damage.is_empty() || moved.is_some(),
                damage,
                moved,
                pixels_per_point: window_scale,
            });
        }
        let window_overflows = std::mem::take(&mut *self.inner.overflows.borrow_mut());
        FrameOutput {
            windows,
            window_requests: std::mem::take(&mut *self.inner.window_requests.borrow_mut()),
            dump,
            copied_text,
            copied_image: self.inner.copied_image.borrow_mut().take(),
            paste: self.inner.paste.take(),
            file_picks: std::mem::take(&mut *self.inner.file_pick_requests.borrow_mut()),
            file_saves: std::mem::take(&mut *self.inner.file_save_requests.borrow_mut()),
            repaint_after: self.inner.repaint_after.get(),
            animating: self.inner.animating.get(),
            fullscreen: self.inner.fullscreen.get(),
            close_requested: self.inner.close_requested.get(),
            repaint: self.inner.repaint.get(),
            accessibility,
            actions: std::mem::take(&mut *self.inner.actions.borrow_mut()),
            unran_actions: std::mem::take(&mut *self.inner.action_requests.borrow_mut())
                .into_iter()
                .map(|(_, id)| id)
                .collect(),
            hits: self
                .inner
                .hit_query
                .take()
                .map(|_| std::mem::take(&mut *self.inner.hit_answers.borrow_mut())),
            comps: self
                .inner
                .comps_query
                .take()
                .map(|_| std::mem::take(&mut *self.inner.comps_answers.borrow_mut())),
            overflows: window_overflows
                .iter()
                .map(|(_, found)| found.clone())
                .collect(),
            window_overflows,
            typed: self.inner.keyboard.borrow_mut().take_typed(),
            pixels_per_point: scale,
        }
    }

    pub fn run(&self, frame: impl FnOnce(&Self)) -> FrameOutput {
        self.run_with(Vec::new(), self.frame_time(), frame)
    }

    pub(crate) fn run_with(
        &self,
        events: Vec<Event>,
        now: Instant,
        frame: impl FnOnce(&Self),
    ) -> FrameOutput {
        self.begin_frame(events, now);
        frame(self);
        self.end_frame()
    }

    pub fn input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R {
        reader(&self.inner.input.borrow())
    }

    pub fn set_ime_area(&self, area: Option<ImeArea>) {
        self.inner.window.borrow_mut().requests.ime = area;
    }

    pub fn ime_area(&self) -> Option<ImeArea> {
        self.inner.window.borrow().requests.ime.clone()
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
        self.inner.window.borrow_mut().requests.handles_back = true;
    }

    pub fn want_keyboard(&self) {
        self.inner.window.borrow_mut().requests.wants_keyboard = true;
    }

    pub fn intercept_keys(&self, chords: &[KeyChord]) {
        let mut window = self.inner.window.borrow_mut();
        for chord in chords {
            if !window.requests.intercepted_keys.contains(chord) {
                window.requests.intercepted_keys.push(*chord);
            }
        }
    }

    pub fn painter(&self) -> Painter {
        Painter::new(self.clone(), Rect::EVERYTHING)
    }

    pub fn copy_text(&self, text: String) {
        *self.inner.copied_image.borrow_mut() = None;
        *self.inner.copied_text.borrow_mut() = Some(text);
    }

    pub fn copy_image(&self, png: Vec<u8>) {
        *self.inner.copied_text.borrow_mut() = None;
        *self.inner.copied_image.borrow_mut() = Some(png);
    }

    pub fn request_paste(&self) {
        if self.inner.paste.get().is_none() {
            self.inner.paste.set(Some(PasteRequest::Asked));
        }
    }

    pub fn request_typed_paste(&self) {
        self.inner.paste.set(Some(PasteRequest::Typed));
    }

    pub fn paste_shortcut(&self) {
        self.inner.paste.set(Some(PasteRequest::Shortcut));
    }

    pub fn accept_pastes(&self, kinds: &[PasteKind]) {
        self.inner.window.borrow_mut().requests.pastes = kinds.to_vec();
    }

    pub fn take_clipboard_keys(&self, keys: bool) {
        self.inner.window.borrow_mut().requests.clipboard_keys = keys;
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

    pub fn save_file(&self, file: SavedFile) -> FileSaveId {
        let id = FileSaveId(self.inner.next_file_save.get() + 1);
        self.inner.next_file_save.set(id.0);
        self.inner
            .file_save_requests
            .borrow_mut()
            .push(FileSaveRequest { id, file });
        id
    }

    pub fn file_saved(&self, id: FileSaveId, save: FileSave) {
        self.inner.file_saves.borrow_mut().push((id, save));
    }

    pub fn take_file_save(&self, id: FileSaveId) -> Option<FileSave> {
        let mut saves = self.inner.file_saves.borrow_mut();
        let index = saves.iter().position(|(saved, _)| *saved == id)?;
        Some(saves.remove(index).1)
    }

    pub fn set_pointer_locked(&self, locked: bool) {
        let changed = std::mem::replace(
            &mut self.inner.window.borrow_mut().requests.pointer_locked,
            locked,
        ) != locked;
        if changed {
            self.request_repaint();
        }
    }

    pub fn pointer_locked(&self) -> bool {
        self.inner.window.borrow().requests.pointer_locked
    }

    pub fn set_cursor_icon(&self, cursor_icon: CursorIcon) {
        self.inner.window.borrow_mut().requests.cursor = cursor_icon;
    }

    pub fn set_appearance(&self, appearance: &Appearance) {
        let mut window = self.inner.window.borrow_mut();
        if window.appearance != *appearance {
            window.appearance.clone_from(appearance);
        }
    }

    pub fn cursor_icon(&self) -> CursorIcon {
        self.inner.window.borrow().requests.cursor
    }

    pub fn request_window(&self, request: WindowRequest) {
        self.inner.window_requests.borrow_mut().push(request);
    }

    pub fn opens_windows(&self) -> bool {
        self.inner.window_hosting.get() == WindowHosting::Any
    }

    pub fn window_hosting(&self) -> WindowHosting {
        self.inner.window_hosting.get()
    }

    pub fn set_window_hosting(&self, hosting: WindowHosting) {
        self.inner.window_hosting.set(hosting);
    }

    pub fn shows_main(&self) -> bool {
        self.inner.shows_main.get()
    }

    pub fn set_shows_main(&self, shows: bool) {
        self.inner.shows_main.set(shows);
    }

    pub fn window_size(&self, window: WindowId) -> Option<Vec2> {
        self.inner
            .windows
            .borrow()
            .get(&window)
            .and_then(|known| known.size)
    }

    pub fn window_pixels_per_point(&self, window: WindowId) -> f32 {
        self.inner
            .windows
            .borrow()
            .get(&window)
            .and_then(|known| known.pixels_per_point)
            .filter(|scale| *scale > 0.0 && scale.is_finite())
            .unwrap_or_else(|| self.inner.pixels_per_point.get())
    }

    pub fn window_painter(&self, window: WindowId) -> Painter {
        let raster = self.window_pixels_per_point(window) / self.inner.pixels_per_point.get();
        let painter = self.painter();
        match raster == 1.0 {
            true => painter,
            false => Painter::resumed(
                self.clone(),
                PainterState {
                    raster,
                    ..painter.state()
                },
            ),
        }
    }

    pub fn window_placement(&self, window: WindowId) -> Option<HostPlacement> {
        self.inner
            .windows
            .borrow()
            .get(&window)
            .and_then(|known| known.placement.clone())
    }

    pub fn window_reported(&self, report: WindowReport) {
        let mut windows = self.inner.windows.borrow_mut();
        let changed = match &report {
            WindowReport::Resized { window, size } => {
                let known = &mut windows.entry(*window).or_default().size;
                known.replace(*size) != Some(*size)
            }
            WindowReport::Placed { window, placement } => {
                let known = &mut windows.entry(*window).or_default().placement;
                known.replace(placement.clone()).as_ref() != Some(placement)
            }
            WindowReport::Scaled {
                window,
                pixels_per_point,
            } => {
                let known = &mut windows.entry(*window).or_default().pixels_per_point;
                known.replace(*pixels_per_point) != Some(*pixels_per_point)
            }
            WindowReport::Closed(window) => {
                windows.remove(window);
                true
            }
            WindowReport::Focused { .. } => true,
        };
        if changed {
            self.inner.window_reports.borrow_mut().push(report);
        }
    }

    pub fn take_window_reports(&self) -> Vec<WindowReport> {
        std::mem::take(&mut *self.inner.window_reports.borrow_mut())
    }

    pub fn set_keyboard_window(&self, window: WindowId) {
        self.inner.keyboard_window.set(window);
    }

    pub fn keyboard_window(&self) -> WindowId {
        self.inner.keyboard_window.get()
    }

    pub fn show_window(
        &self,
        id: WindowId,
        size: Vec2,
        painting: &[(Rc<Display>, Vec2)],
        damage: Region,
        moved: Option<Moved>,
        title: Option<String>,
        appearance: Option<&Appearance>,
    ) {
        let layers = painting
            .iter()
            .map(|(display, at)| Layer::Display(*at, Rc::clone(display)))
            .collect();
        self.inner.shown_windows.borrow_mut().push(ShownWindow {
            id,
            size,
            layers,
            damage,
            moved,
            title,
            appearance: appearance.cloned(),
        });
    }

    fn window_states(&self, mut main: WindowState) -> (WindowState, Vec<WindowState>) {
        let pointer = self.inner.input.borrow().pointer.window;
        let keyboard = self.inner.keyboard_window.get();
        let shown = self.inner.shown_windows.borrow();
        let mut others = Vec::with_capacity(shown.len());
        for window in shown.iter() {
            let mut state = WindowState {
                typed: main.typed,
                ..WindowState::default()
            };
            let own = window.appearance.as_ref().unwrap_or(&main.appearance);
            state.appearance.clear_color = own.clear_color;
            state.appearance.system_bars = own.system_bars;
            state.appearance.title.clone_from(&window.title);
            if pointer == window.id {
                state.requests.cursor = main.requests.cursor;
                state.requests.pointer_locked = main.requests.pointer_locked;
            }
            if keyboard == window.id {
                state.requests.ime = main.requests.ime.take();
            }
            others.push(state);
        }
        if shown.iter().any(|window| window.id == pointer) {
            main.requests.cursor = CursorIcon::default();
        }
        (main, others)
    }

    fn shown_window(&self) -> WindowState {
        let mut window = self.inner.window.borrow().clone();
        window.requests.keyboard_toggles = self.inner.keyboard.borrow().toggles();
        if self.touch_emulation() {
            window.requests.cursor = CursorIcon::Touch;
        }
        window
    }

    pub fn preferences(&self) -> Preferences {
        self.inner.preferences.get().resolve()
    }

    pub fn motion(&self) -> crate::animation::Motion {
        self.preferences().motion
    }

    pub fn preference_layers(&self) -> PreferenceLayers {
        self.inner.preferences.get()
    }

    pub fn preference_layer(&self, source: PreferenceSource) -> PreferenceLayer {
        self.inner.preferences.get().layer(source)
    }

    pub fn set_preference_layer(&self, source: PreferenceSource, layer: PreferenceLayer) {
        let mut layers = self.inner.preferences.get();
        if layers.layer(source) == layer {
            return;
        }
        *layers.layer_mut(source) = layer;
        self.inner.preferences.set(layers);
        self.request_repaint();
    }

    pub fn touch_emulation(&self) -> bool {
        self.inner.touch_emulation.get()
    }

    pub fn set_touch_emulation(&self, enabled: bool) {
        self.inner.touch_emulation.set(enabled);
    }

    pub fn enable_keyboard(&self) {
        self.inner.keyboard.borrow_mut().enable();
    }

    pub fn keyboard_enabled(&self) -> bool {
        self.inner.keyboard.borrow().enabled()
    }

    pub fn keyboard_attached(&self) -> bool {
        self.inner.keyboard.borrow().attached()
    }

    pub fn set_keyboard_attached(&self, attached: bool) {
        self.inner.keyboard.borrow_mut().set_attached(attached);
    }

    pub fn keyboard_share(&self) -> f32 {
        self.inner.keyboard.borrow().share()
    }

    pub fn set_keyboard_share(&self, share: f32) {
        if self.inner.keyboard.borrow_mut().set_share(share) {
            self.request_repaint();
        }
    }

    pub fn toggle_keyboard(&self) {
        self.inner.keyboard.borrow_mut().toggle();
        self.request_repaint();
    }

    pub fn close_keyboard(&self) {
        self.inner.keyboard.borrow_mut().close();
        self.request_repaint();
    }

    pub fn keyboard_opened(&self) -> crate::software_keyboard::Opened {
        self.inner.keyboard.borrow().opened()
    }

    pub fn keyboard_rect(&self) -> Option<Rect> {
        self.inner.keyboard.borrow().rect()
    }

    pub fn type_events(&self, events: impl IntoIterator<Item = crate::input::Input>) {
        self.inner.keyboard.borrow_mut().type_events(events);
        self.request_repaint();
    }

    pub(crate) fn keyboard<R>(
        &self,
        change: impl FnOnce(&mut crate::software_keyboard::KeyboardState) -> R,
    ) -> R {
        change(&mut self.inner.keyboard.borrow_mut())
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

    pub fn set_waker(&self, waker: Waker) {
        *self.inner.waker.borrow_mut() = Some(waker);
        let inner = Rc::downgrade(&self.inner);
        ::reactive::wake_with(move || {
            if let Some(inner) = inner.upgrade()
                && !inner.in_frame.get()
            {
                inner.wake();
            }
        });
    }

    pub(crate) fn zone_waker(&self, zone: u64) -> impl Fn() + 'static {
        let inner = Rc::downgrade(&self.inner);
        move || {
            let Some(inner) = inner.upgrade() else {
                return;
            };
            if inner.in_frame.get() {
                inner.zones_woken_in_frame.borrow_mut().push(zone);
                return;
            }
            inner.wake();
        }
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

    pub fn note_animating(&self) {
        self.inner.animating.set(true);
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

    pub fn paint_child(&self, scope: &Option<Rc<Scope>>, id: NodeId, at: Vec2) {
        if let Some(frame) = self.inner.paint_stack.borrow_mut().last_mut()
            && frame.id.is_some()
        {
            frame.items.push(Item {
                scope: scope.clone(),
                recording: Recording::Child(id, at),
            });
        }
    }

    pub fn report_move(&self, moved: Moved, roots: Vec<Rc<Display>>) {
        self.inner.moves.borrow_mut().push((moved, roots));
    }

    fn settle_moves(&self, layers: &[Layer], pixels_per_point: f32) -> Option<Moved> {
        let moves = std::mem::take(&mut *self.inner.moves.borrow_mut());
        let settled = match moves.as_slice() {
            [(moved, roots)]
                if whole_pixels(moved.by, pixels_per_point)
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
        let pixels_per_point = self.inner.pixels_per_point.get();
        self.inner.pixels_per_point.set(pixels_per_point * scale);
        let input = self
            .inner
            .input
            .replace_with(|input| input.scaled(scale.recip()));
        let layers = self.inner.layers.borrow().len();
        let damage = self.inner.damage.borrow().len();
        let moves = self.inner.moves.borrow().len();
        let fragments = self.inner.accessibility.borrow().len();
        let ime = self.inner.window.borrow_mut().requests.ime.take();
        let result = self.grouping(layers, content);
        self.unmove(moves);
        let scaled_ime = std::mem::replace(&mut self.inner.window.borrow_mut().requests.ime, ime);
        if let Some(area) = scaled_ime {
            self.inner.window.borrow_mut().requests.ime = Some(ImeArea {
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

    pub fn rastered(&self, galley: &Galley, raster: f32) -> Galley {
        self.inner.fonts.borrow_mut().rastered(galley, raster)
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

fn whole_pixels(by: Vec2, pixels_per_point: f32) -> bool {
    let whole =
        |value: f32| ((value * pixels_per_point).round() - value * pixels_per_point).abs() < 0.01;
    whole(by.x) && whole(by.y)
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
                    viewport,
                    offset,
                    scale,
                    ..
                } => area
                    .intersect(viewport)
                    .translate(-offset)
                    .scaled(scale.recip()),
                Group::Mask { .. } | Group::Filter(_) => area,
            };
            within.is_positive() && overlaps_besides(inner, roots, within)
        }
    })
}

struct Previous {
    pixels_per_point: f32,
    windows: Vec<(WindowId, f32)>,
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
        | Shape::Drawing { clip, .. }
        | Shape::Path { clip, .. } => clip,
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
            scale: held,
            rotation,
            clip,
            ..
        } => {
            *origin = pos2(origin.x * scale, origin.y * scale);
            *held *= scale;
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
        Shape::Path {
            scale: held,
            offset,
            rotation,
            clip,
            ..
        } => {
            *held *= scale;
            *offset *= scale;
            *rotation = rotation.scaled(scale);
            *clip = clip.scaled(scale);
        }
    }
}
