use beui::{Input, Key, KeyChord, NodeId, PointerButton, Pos2, Rect, Vec2, vec2};
use beui_host::native::drive::gestures::Frames;
use block_editor_beui::{BeuiApp, View, ViewId};
use block_plugin_api::{
    AnchorSpace, HostTransform, InputBatch, ScreenId, ScreenRequest, TimedInput, ViewRef,
    ViewportMetrics, WindowOpen, WindowRequest,
};
use window_stack::{Gesture, Host, Stacked};

use super::{BeuiTest, INSTANCE, SCREEN};

const FIRST_DISPLAY_SCREEN: u64 = 100;
const FIRST_WINDOW_SCREEN: u64 = 1000;

#[derive(Clone, Copy, Debug)]
struct Display {
    rect: Rect,
    scale: f32,
    screen: ScreenId,
    work: Option<Rect>,
}

#[derive(Clone, Debug)]
struct Hosted {
    window: ViewId,
    screen: ScreenId,
    rect: Rect,
    parent: Option<ViewId>,
}

#[derive(Clone, Debug)]
struct Anchored {
    layer: u64,
    screen: ScreenId,
    rect: Rect,
    takes: Vec<Rect>,
}

#[derive(Default)]
pub(crate) struct Desk {
    displays: Vec<Display>,
    windows: Vec<Hosted>,
    anchored: Vec<Anchored>,
    next_screen: u64,
    gesture: Option<(ViewId, Gesture)>,
    pressed: Option<(ScreenId, Pos2, Option<(ViewId, Rect)>)>,
    pointer: Pos2,
    outside: bool,
    intercepts: Vec<(ScreenId, Vec<KeyChord>)>,
}

impl Desk {
    fn new(displays: &[(Rect, f32)]) -> Self {
        Self {
            displays: displays
                .iter()
                .enumerate()
                .map(|(index, (rect, scale))| Display {
                    rect: *rect,
                    scale: *scale,
                    screen: match index {
                        0 => SCREEN,
                        index => ScreenId(FIRST_DISPLAY_SCREEN + index as u64),
                    },
                    work: None,
                })
                .collect(),
            next_screen: FIRST_WINDOW_SCREEN,
            ..Self::default()
        }
    }

    fn reported(&mut self, reports: &[block_plugin_api::FrameReport]) -> bool {
        let mut changed = false;
        for display in &mut self.displays {
            let Some(report) = reports
                .iter()
                .find(|report| report.screen == display.screen)
            else {
                continue;
            };
            let work = report.work_area.map(|insets| insets.inside(display.rect));
            changed |= display.work != work;
            display.work = work;
        }
        changed
    }

    fn keep(&mut self) -> bool {
        let moving = self
            .gesture
            .and_then(|(window, _)| self.windows.iter().position(|held| held.window == window));
        let displays = &self.displays;
        window_stack::keep_in_work_areas(&mut self.windows, moving, |held| {
            let display = owner(displays, held.rect);
            Some((display.rect, display.work?))
        })
    }

    fn origin(&self, screen: ScreenId) -> Pos2 {
        self.displays
            .iter()
            .find(|display| display.screen == screen)
            .map(|display| display.rect.min)
            .or_else(|| {
                self.windows
                    .iter()
                    .find(|window| window.screen == screen)
                    .map(|window| window.rect.min)
            })
            .or_else(|| {
                self.anchored
                    .iter()
                    .find(|anchored| anchored.screen == screen)
                    .map(|anchored| anchored.rect.min)
            })
            .unwrap_or(Pos2::ZERO)
    }

    fn view_screen(&self, view: ViewId) -> Option<ScreenId> {
        if view.is_main() {
            return Some(SCREEN);
        }
        let display = (1..self.displays.len()).find(|index| ViewId::display(*index as u32) == view);
        match display {
            Some(index) => Some(self.displays[index].screen),
            None => self
                .windows
                .iter()
                .find(|held| held.window == view)
                .map(|held| held.screen),
        }
    }

    fn under(&self, at: Pos2) -> Option<ScreenId> {
        self.anchored
            .iter()
            .rev()
            .find(|anchored| anchored.takes.iter().any(|rect| rect.contains(at)))
            .map(|anchored| anchored.screen)
            .or_else(|| {
                self.windows
                    .iter()
                    .rev()
                    .find(|window| window.rect.contains(at))
                    .map(|window| window.screen)
            })
            .or_else(|| {
                self.displays
                    .iter()
                    .find(|display| display.rect.contains(at))
                    .map(|display| display.screen)
            })
    }

    fn requests(&self, frame: Option<block_plugin_api::FrameSpec>) -> Vec<ScreenRequest> {
        let metrics = |size: Vec2, scale: f32| ViewportMetrics {
            logical: block_plugin_api::Size {
                width: size.x,
                height: size.y,
            },
            visible: block_plugin_api::Pos2::ZERO,
            pixel_width: (size.x * scale).round() as u32,
            pixel_height: (size.y * scale).round() as u32,
            scale_factor: scale,
            raster: scale,
        };
        let displays = self
            .displays
            .iter()
            .enumerate()
            .map(|(index, display)| {
                let shown = match index {
                    0 => View::MAIN,
                    index => View::display(index as u32),
                };
                (index, display, shown)
            })
            .map(|(index, display, shown)| ScreenRequest {
                screen: display.screen,
                instance: INSTANCE,
                view: shown.id,
                placement: shown.placement,
                metrics: metrics(display.rect.size(), display.scale),
                frame: match index {
                    0 => frame,
                    _ => None,
                },
                windows: true,
                host_place: None,
                origin: Some(display.rect.min),
                docked: false,
                anchor_space: Some(AnchorSpace {
                    to_host: HostTransform {
                        origin: display.rect.min,
                        scale: vec2(1.0, 1.0),
                    },
                    bounds: display.rect,
                }),
            });
        let windows = self.windows.iter().map(|window| {
            let display = owner(&self.displays, window.rect);
            ScreenRequest {
                screen: window.screen,
                instance: INSTANCE,
                view: window.window,
                placement: View::window(window.window).placement,
                metrics: metrics(window.rect.size(), display.scale),
                frame: None,
                windows: true,
                host_place: Some(window_stack::placement(window.rect.min)),
                origin: Some(window.rect.min),
                docked: false,
                anchor_space: Some(AnchorSpace {
                    to_host: HostTransform {
                        origin: window.rect.min,
                        scale: vec2(1.0, 1.0),
                    },
                    bounds: display.rect,
                }),
            }
        });
        displays.chain(windows).collect()
    }

    fn ask(&mut self, request: WindowRequest) {
        let mut windows = std::mem::take(&mut self.windows);
        if let Some((index, gesture)) = window_stack::ask(&mut windows, self, request) {
            self.gesture = Some((windows[index].window, gesture));
        }
        self.windows = windows;
    }

    fn raise(&mut self, screen: ScreenId) {
        if let Some(index) = self.windows.iter().position(|held| held.screen == screen) {
            window_stack::raise(&mut self.windows, index);
        }
    }

    fn follow(&mut self, at: Pos2) -> bool {
        let Some((window, gesture)) = self.gesture else {
            return false;
        };
        let Some((_, pressed, Some((held, rect)))) = self.pressed else {
            return false;
        };
        let Some(index) = self.windows.iter().position(|held| held.window == window) else {
            return false;
        };
        if held != window {
            return false;
        }
        window_stack::drag(&mut self.windows, index, gesture, rect, at - pressed);
        true
    }
}

impl Stacked for Hosted {
    fn view(&self) -> ViewId {
        self.window
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn opened_over(&self, other: &Self) -> bool {
        self.parent == Some(other.window)
    }
}

impl Host<Hosted> for Desk {
    fn mine(&self, _: &Hosted) -> bool {
        true
    }

    fn pointer(&self) -> Pos2 {
        self.pointer
    }

    fn over(&self, view: ViewRef, at: Pos2) -> Pos2 {
        let screen = self.view_screen(view.view);
        screen.map_or(Pos2::ZERO, |screen| self.origin(screen)) + at.to_vec2()
    }

    fn open(
        &mut self,
        open: &WindowOpen,
        rect: Rect,
        held: Option<&Hosted>,
        opener: Option<&Hosted>,
    ) -> Hosted {
        if let Some(held) = held {
            return Hosted {
                rect,
                ..held.clone()
            };
        }
        let screen = match open.screen.is_opened() {
            true => open.screen,
            false => {
                self.next_screen += 1;
                ScreenId(self.next_screen)
            }
        };
        Hosted {
            window: open.view,
            screen,
            rect,
            parent: opener.map(|opener| opener.window),
        }
    }
}

fn owner(displays: &[Display], rect: Rect) -> Display {
    let area = |display: &Display| {
        let shared = display.rect.intersect(rect);
        shared.width().max(0.0) * shared.height().max(0.0)
    };
    displays
        .iter()
        .copied()
        .max_by(|a, b| area(a).total_cmp(&area(b)))
        .filter(|display| area(display) > 0.0)
        .unwrap_or(displays[0])
}

impl<A: BeuiApp> BeuiTest<A> {
    pub fn show_on_displays(&mut self, displays: &[(Rect, f32)]) {
        let desk = Desk::new(displays);
        self.size = desk.displays[0].rect.size();
        self.scale_factor = desk.displays[0].scale;
        self.desk = Some(desk);
        self.place();
        self.run();
    }

    pub(super) fn desk_requests(&self) -> Option<Vec<ScreenRequest>> {
        Some(self.desk.as_ref()?.requests(self.frame))
    }

    pub(super) fn desk_escaped(&mut self) {
        let Some(desk) = self.desk.as_mut() else {
            return;
        };
        let Some(output) = self.output.as_ref() else {
            desk.anchored.clear();
            return;
        };
        let screens = &self.plugin.layout().screens;
        desk.anchored = output
            .escaped
            .iter()
            .filter_map(|escaped| {
                let view = beui_platform_plugin_api::anchored_view(escaped.layer.id);
                let screen = screens.iter().find(|placed| placed.view == view)?.screen;
                Some(Anchored {
                    layer: escaped.layer.id,
                    screen,
                    rect: escaped.layer.rect,
                    takes: escaped.layer.floating.clone(),
                })
            })
            .collect();
    }

    pub(super) fn desk_asked(
        &mut self,
        requests: Vec<WindowRequest>,
        reports: &[block_plugin_api::FrameReport],
    ) {
        let Some(desk) = self.desk.as_mut() else {
            return;
        };
        let reported = desk.reported(reports);
        let asked = !requests.is_empty();
        if !asked && !reported {
            return;
        }
        for request in requests {
            desk.ask(request);
        }
        if desk.keep() || asked {
            self.place();
        }
    }

    pub(super) fn desk_intercepts(&mut self, windows: &[block_plugin_api::ScreenWindow]) {
        let Some(desk) = self.desk.as_mut() else {
            return;
        };
        for window in windows {
            let keys = window.window.intercepted_keys.clone();
            match desk
                .intercepts
                .iter_mut()
                .find(|(screen, _)| *screen == window.screen)
            {
                Some((_, held)) => *held = keys,
                None => desk.intercepts.push((window.screen, keys)),
            }
        }
    }

    pub(super) fn desk_tap(&mut self, key: Key) -> Option<bool> {
        let desk = self.desk.as_mut()?;
        let tap = KeyChord::tap(key);
        let screen = desk
            .intercepts
            .iter()
            .find(|(screen, keys)| *screen != SCREEN && keys.contains(&tap))?
            .0;
        self.desk_batches.push(InputBatch {
            screen,
            every_sample: false,
            events: vec![TimedInput::from(Input::InterceptedTap(key))],
            dismissed: Vec::new(),
        });
        Some(true)
    }

    pub fn desk_windows(&self) -> Vec<Rect> {
        self.desk
            .as_ref()
            .map(|desk| desk.windows.iter().map(|window| window.rect).collect())
            .unwrap_or_default()
    }

    pub fn desk_window_scales(&self) -> Vec<f32> {
        self.desk
            .as_ref()
            .map(|desk| {
                desk.windows
                    .iter()
                    .map(|window| owner(&desk.displays, window.rect).scale)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn desk_rect(&self, node: NodeId) -> Option<Rect> {
        let desk = self.desk.as_ref()?;
        let document = self.document();
        let rect = document.node_rect(node)?;
        let window = document.window_of(node);
        drop(document);
        let screen = self
            .drawn_screens
            .iter()
            .find(|(_, held)| *held == window)?
            .0;
        Some(rect.translate(desk.origin(screen).to_vec2()))
    }

    pub fn desk_rects_of(&self, test_id: &str) -> Vec<Rect> {
        let nodes = self.document().find_test_ids(test_id);
        nodes
            .into_iter()
            .filter_map(|node| self.desk_rect(node))
            .collect()
    }

    pub fn desk_press(&mut self, at: Pos2) {
        self.desk_pointer(at, Some(true), PointerButton::Primary);
    }

    pub fn desk_move(&mut self, at: Pos2) {
        self.desk_pointer(at, None, PointerButton::Primary);
    }

    pub fn desk_release(&mut self, at: Pos2) {
        self.desk_pointer(at, Some(false), PointerButton::Primary);
    }

    pub(super) fn desk_point(&self, test_id: &str, offset: Option<Vec2>) -> Option<Pos2> {
        let rect = self.desk_rects_of(test_id).into_iter().next()?;
        Some(match offset {
            Some(offset) => rect.min + offset,
            None => rect.center(),
        })
    }

    pub(super) fn desk_perform(&mut self, frames: Frames) {
        let events: Vec<Input> = frames.into_iter().flatten().collect();
        for (index, event) in events.iter().enumerate() {
            match *event {
                Input::PointerMoved(at) => {
                    let pointer = self.desk.as_ref().map(|desk| desk.pointer);
                    let carried = match events.get(index + 1) {
                        Some(Input::PointerMoved(next)) => *next == at,
                        Some(Input::PointerButton { pos, pressed, .. }) => {
                            *pos == at && (*pressed || pointer == Some(at))
                        }
                        _ => false,
                    };
                    if !carried {
                        self.desk_move(at);
                    }
                }
                Input::PointerButton {
                    pos,
                    button,
                    pressed,
                } => self.desk_pointer(pos, Some(pressed), button),
                ref other => panic!("the desk takes only pointer input, not {other:?}"),
            }
        }
    }

    pub fn desk_drag(&mut self, from: Pos2, to: Pos2, steps: usize) {
        self.desk_press(from);
        self.desk_moves(from, to, steps, std::time::Duration::ZERO);
        self.desk_release(to);
    }

    pub fn desk_moves(&mut self, from: Pos2, to: Pos2, steps: usize, each: std::time::Duration) {
        for step in 1..=steps.max(1) {
            if !each.is_zero() {
                self.advance(each);
            }
            let along = step as f32 / steps.max(1) as f32;
            self.desk_move(from + (to - from) * along);
        }
    }

    fn desk_pointer(&mut self, at: Pos2, pressed: Option<bool>, button: PointerButton) {
        for events in std::mem::take(&mut self.frames) {
            self.step(events);
        }
        let Some(desk) = self.desk.as_mut() else {
            panic!("desk input needs show_on_displays");
        };
        desk.pointer = at;
        let under = desk.under(at);
        let mut events = Vec::new();
        let holder = match (pressed, desk.pressed) {
            (Some(true), _) => {
                let Some(screen) = under else {
                    return;
                };
                desk.raise(screen);
                let window = desk
                    .windows
                    .iter()
                    .find(|held| held.screen == screen)
                    .map(|held| (held.window, held.rect));
                desk.pressed = Some((screen, at, window));
                desk.outside = false;
                screen
            }
            (_, Some((screen, _, _))) => screen,
            (_, None) => match under {
                Some(screen) => screen,
                None => return,
            },
        };
        if pressed.is_none() && desk.follow(at) {
            self.place();
            self.step(Vec::new());
            return;
        }
        let held = desk.pressed.is_some();
        let over = under.filter(|screen| held && *screen != holder);
        if held && under.is_none() && !desk.outside {
            desk.outside = true;
            events.push(TimedInput::from(Input::PointerLeft));
        }
        if under.is_some() {
            desk.outside = false;
        }
        let local = at - desk.origin(over.unwrap_or(holder));
        let input = match pressed {
            Some(pressed) => Input::PointerButton {
                pos: Pos2::ZERO + local,
                button,
                pressed,
            },
            None => Input::PointerMoved(Pos2::ZERO + local),
        };
        events.push(TimedInput {
            input,
            time_micros: Some(self.clock.as_micros() as u64),
            over,
        });
        let ended = pressed == Some(false) && {
            desk.pressed = None;
            desk.gesture.take().is_some()
        };
        if ended {
            desk.keep();
            self.place();
        }
        self.desk_batches.push(InputBatch {
            screen: holder,
            every_sample: true,
            events,
            dismissed: Vec::new(),
        });
        self.step(Vec::new());
    }

    pub fn desk_snapshot(&mut self, name: &str, display: usize) {
        let desk = self
            .desk
            .as_ref()
            .expect("desk paintings need show_on_displays");
        let shown = desk.displays[display];
        let output = self
            .output
            .as_ref()
            .expect("the editor has not drawn a frame yet");
        let behind = beui::styled::Theme::of(self.document().preferences().theme).background;
        let painting_of = |screen: ScreenId, size: Vec2| {
            let window = self
                .drawn_screens
                .iter()
                .find(|(held, _)| *held == screen)
                .map(|(_, window)| *window)?;
            let window = output.window(window)?;
            beui::native::painting::painting(
                window,
                size,
                shown.scale,
                window.window.appearance.clear_color,
                beui::native::painting::Punches::Shown,
            )
            .ok()
        };
        let mut painting = painting_of(shown.screen, shown.rect.size())
            .unwrap_or_else(|| panic!("display {display} has drawn nothing yet"));
        if painting.frame.background[3] == 0 {
            painting.frame.background = behind.to_array();
        }
        for window in &desk.windows {
            if !window.rect.intersects(shown.rect) {
                continue;
            }
            let Some(drawn) = painting_of(window.screen, window.rect.size()) else {
                continue;
            };
            let at = window.rect.translate(-shown.rect.min.to_vec2());
            painting
                .frame
                .primitives
                .extend(drawn.placed(beui::native::painting::Placement {
                    source: [0.0, 0.0, 1.0, 1.0],
                    corners: [
                        [at.min.x, at.min.y],
                        [at.max.x, at.min.y],
                        [at.max.x, at.max.y],
                        [at.min.x, at.max.y],
                    ],
                    clip: [0.0, 0.0, shown.rect.width(), shown.rect.height()],
                    corner_radius: 0.0,
                }));
            for (key, pixels) in drawn.textures {
                painting.textures.entry(key).or_insert(pixels);
            }
        }
        for anchored in &desk.anchored {
            let rect = anchored.rect;
            if !rect.intersects(shown.rect) {
                continue;
            }
            let Some(escaped) = output
                .escaped
                .iter()
                .find(|escaped| escaped.layer.id == anchored.layer)
            else {
                continue;
            };
            let Ok(drawn) = beui::native::painting::painting(
                &escaped.output,
                rect.size(),
                shown.scale,
                escaped.output.window.appearance.clear_color,
                beui::native::painting::Punches::Shown,
            ) else {
                continue;
            };
            let at = rect.translate(-shown.rect.min.to_vec2());
            painting
                .frame
                .primitives
                .extend(drawn.placed(beui::native::painting::Placement {
                    source: [0.0, 0.0, 1.0, 1.0],
                    corners: [
                        [at.min.x, at.min.y],
                        [at.max.x, at.min.y],
                        [at.max.x, at.max.y],
                        [at.min.x, at.max.y],
                    ],
                    clip: [0.0, 0.0, shown.rect.width(), shown.rect.height()],
                    corner_radius: 0.0,
                }));
            for (key, pixels) in drawn.textures {
                painting.textures.entry(key).or_insert(pixels);
            }
        }
        paint_snapshot::assert_painting(name, painting);
    }
}
