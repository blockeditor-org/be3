use beui::{Input, NodeId, PointerButton, Pos2, Rect, Vec2, pos2, vec2};
use block_editor_beui::{BeuiApp, View, ViewId};
use block_plugin_api::{
    InputBatch, ScreenId, ScreenRequest, TimedInput, ViewportMetrics, WindowEdge, WindowPlacement,
    WindowRequest,
};

use super::{BeuiTest, INSTANCE, SCREEN};

const OPEN_GRAB: Vec2 = vec2(24.0, 16.0);
const FIRST_DISPLAY_SCREEN: u64 = 100;
const FIRST_WINDOW_SCREEN: u64 = 1000;
const WALL: f32 = 0.5;

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

#[derive(Clone, Copy, Debug)]
enum Gesture {
    Move,
    Resize(WindowEdge),
}

#[derive(Default)]
pub(crate) struct Desk {
    displays: Vec<Display>,
    windows: Vec<Hosted>,
    next_screen: u64,
    gesture: Option<(ViewId, Gesture)>,
    pressed: Option<(ScreenId, Pos2, Option<(ViewId, Rect)>)>,
    pointer: Pos2,
    outside: bool,
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

    fn owner(&self, rect: Rect) -> Display {
        let area = |display: &Display| {
            let shared = display.rect.intersect(rect);
            shared.width().max(0.0) * shared.height().max(0.0)
        };
        self.displays
            .iter()
            .copied()
            .max_by(|a, b| area(a).total_cmp(&area(b)))
            .filter(|display| area(display) > 0.0)
            .unwrap_or(self.displays[0])
    }

    fn reported(&mut self, reports: &[block_plugin_api::FrameReport]) -> bool {
        let mut changed = false;
        for display in &mut self.displays {
            let Some(report) = reports.iter().find(|report| report.screen == display.screen)
            else {
                continue;
            };
            let work = report
                .work_area
                .map(|insets| insets.inside(display.rect));
            changed |= display.work != work;
            display.work = work;
        }
        changed
    }

    fn keep(&mut self) -> bool {
        let moving = self.gesture.map(|(window, _)| window);
        let mut moved = false;
        for index in 0..self.windows.len() {
            let held = &self.windows[index];
            if Some(held.window) == moving {
                continue;
            }
            let display = self.owner(held.rect);
            let Some(work) = display.work else {
                continue;
            };
            let to = walled(held.rect, display.rect, work);
            if to != held.rect.min {
                let size = held.rect.size();
                self.windows[index].rect = Rect::from_min_size(to, size);
                moved = true;
            }
        }
        moved
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
        self.windows
            .iter()
            .rev()
            .find(|window| window.rect.contains(at))
            .map(|window| window.screen)
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
                anchor_space: None,
            });
        let windows = self.windows.iter().map(|window| ScreenRequest {
            screen: window.screen,
            instance: INSTANCE,
            view: window.window,
            placement: View::window(window.window).placement,
            metrics: metrics(window.rect.size(), self.owner(window.rect).scale),
            frame: None,
            windows: true,
            host_place: Some(placement(window.rect)),
            origin: Some(window.rect.min),
            docked: false,
            anchor_space: None,
        });
        displays.chain(windows).collect()
    }

    fn ask(&mut self, request: WindowRequest) {
        let find = |windows: &[Hosted], window: ViewId| {
            windows.iter().position(|held| held.window == window)
        };
        match request {
            WindowRequest::Open(open) => {
                let origin = self.point_of(open.placement);
                let rect = Rect::from_min_size(origin, vec2(open.size.width, open.size.height));
                let parent = find(&self.windows, open.opener.view).map(|_| open.opener.view);
                match find(&self.windows, open.view) {
                    Some(index) => self.windows[index].rect = rect,
                    None => {
                        let screen = match open.screen.is_opened() {
                            true => open.screen,
                            false => {
                                self.next_screen += 1;
                                ScreenId(self.next_screen)
                            }
                        };
                        self.windows.push(Hosted {
                            window: open.view,
                            screen,
                            rect,
                            parent,
                        });
                    }
                }
            }
            WindowRequest::BeginMove(window) => self.gesture = Some((window, Gesture::Move)),
            WindowRequest::Raise(window) => {
                if let Some(index) = find(&self.windows, window) {
                    let raised = self.windows.remove(index);
                    self.windows.push(raised);
                }
            }
            WindowRequest::BeginResize(resize) => {
                self.gesture = Some((resize.view, Gesture::Resize(resize.edge)));
            }
            WindowRequest::Resize(resize) => {
                if let Some(index) = find(&self.windows, resize.view) {
                    let held = &mut self.windows[index];
                    held.rect = Rect::from_min_size(
                        held.rect.min,
                        vec2(resize.size.width, resize.size.height),
                    );
                }
            }
            WindowRequest::Close(window) => {
                if let Some(index) = find(&self.windows, window) {
                    self.windows.remove(index);
                }
            }
            WindowRequest::Place(place) => {
                let origin = self.point_of(place.placement);
                self.move_to(place.view, origin);
            }
        }
    }

    fn point_of(&self, placement: WindowPlacement) -> Pos2 {
        match placement {
            WindowPlacement::Over(over) => {
                let screen = self.view_screen(over.view.view);
                screen.map_or(Pos2::ZERO, |screen| self.origin(screen)) + over.at.to_vec2()
            }
            WindowPlacement::Host(placement) => {
                read(&placement).unwrap_or(self.pointer - OPEN_GRAB)
            }
            WindowPlacement::AtPointer => self.pointer - OPEN_GRAB,
        }
    }

    fn raise(&mut self, screen: ScreenId) {
        if let Some(index) = self.windows.iter().position(|held| held.screen == screen) {
            let held = self.windows.remove(index);
            self.windows.push(held);
        }
    }

    fn follow(&mut self, at: Pos2) -> bool {
        let Some((window, gesture)) = self.gesture else {
            return false;
        };
        let Some((_, pressed, Some((held, rect)))) = self.pressed else {
            return false;
        };
        if held != window {
            return false;
        }
        let moved = at - pressed;
        match gesture {
            Gesture::Move => self.move_to(window, rect.min + moved),
            Gesture::Resize(edge) => {
                if let Some(window) = self.windows.iter_mut().find(|held| held.window == window) {
                    window.rect = resized(rect, edge, moved);
                }
            }
        }
        true
    }

    fn move_to(&mut self, window: ViewId, to: Pos2) {
        let Some(from) = self
            .windows
            .iter()
            .find(|held| held.window == window)
            .map(|held| held.rect.min)
        else {
            return;
        };
        let mut family = vec![window];
        let mut index = 0;
        while index < family.len() {
            let parent = family[index];
            family.extend(
                self.windows
                    .iter()
                    .filter(|held| held.parent == Some(parent) && !family.contains(&held.window))
                    .map(|held| held.window)
                    .collect::<Vec<_>>(),
            );
            index += 1;
        }
        for held in &mut self.windows {
            if family.contains(&held.window) {
                held.rect = held.rect.translate(to - from);
            }
        }
    }
}

fn walled(rect: Rect, display: Rect, work: Rect) -> Pos2 {
    let mut min = rect.min;
    if work.max.x < display.max.x - WALL {
        min.x = min.x.min(work.max.x - rect.width()).max(work.min.x.min(rect.min.x));
    }
    if work.max.y < display.max.y - WALL {
        min.y = min.y.min(work.max.y - rect.height()).max(work.min.y.min(rect.min.y));
    }
    if work.min.x > display.min.x + WALL {
        min.x = min.x.max(work.min.x);
    }
    if work.min.y > display.min.y + WALL {
        min.y = min.y.max(work.min.y);
    }
    min
}

fn resized(rect: Rect, edge: WindowEdge, moved: Vec2) -> Rect {
    let (mut min, mut max) = (rect.min, rect.max);
    match edge {
        WindowEdge::Top | WindowEdge::TopLeft | WindowEdge::TopRight => min.y += moved.y,
        WindowEdge::Bottom | WindowEdge::BottomLeft | WindowEdge::BottomRight => max.y += moved.y,
        WindowEdge::Left | WindowEdge::Right => {}
    }
    match edge {
        WindowEdge::Left | WindowEdge::TopLeft | WindowEdge::BottomLeft => min.x += moved.x,
        WindowEdge::Right | WindowEdge::TopRight | WindowEdge::BottomRight => max.x += moved.x,
        WindowEdge::Top | WindowEdge::Bottom => {}
    }
    Rect::from_min_max(min, max)
}

fn placement(rect: Rect) -> String {
    format!("desktop {} {}", rect.min.x, rect.min.y)
}

fn read(placement: &str) -> Option<Pos2> {
    let mut words = placement.split_whitespace();
    (words.next()? == "desktop").then_some(())?;
    Some(pos2(
        words.next()?.parse().ok()?,
        words.next()?.parse().ok()?,
    ))
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
                    .map(|window| desk.owner(window.rect).scale)
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
        self.desk_pointer(at, Some(true));
    }

    pub fn desk_move(&mut self, at: Pos2) {
        self.desk_pointer(at, None);
    }

    pub fn desk_release(&mut self, at: Pos2) {
        self.desk_pointer(at, Some(false));
    }

    pub fn desk_drag(&mut self, from: Pos2, to: Pos2, steps: usize) {
        self.desk_press(from);
        for step in 1..=steps.max(1) {
            let along = step as f32 / steps.max(1) as f32;
            self.desk_move(from + (to - from) * along);
        }
        self.desk_release(to);
    }

    fn desk_pointer(&mut self, at: Pos2, pressed: Option<bool>) {
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
                button: PointerButton::Primary,
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
        let snapshot = crate::snapshot::Snapshot::of_painting(painting)
            .expect("the display's painting could not be rendered");
        crate::snapshot::assert_snapshot(name, &snapshot);
    }
}
