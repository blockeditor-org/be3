use std::any::Any;
use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::pin;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};
use std::time::Duration;

use crate::app::automation::keys;
use crate::app::automation::{Inbox, Reply, Resize, Simulation, WindowSize, ask};
use crate::app::{App, SafeArea, Setup, Waker};
use crate::color::Color32;
use crate::context::RendererInfo;
use crate::context::{Context, FrameOutput, WindowOutput};
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{Input, Key, Modifiers, PointerButton, TouchPhase};
use crate::renderer::{Loaded, Renderer};
use crate::runner::{HostView, Hosted, Incoming, Launch, Platform, RunOptions, Runner, WindowHost};
use crate::window::{HostPlacement, WindowId, WindowPlacement, WindowReport, WindowRequest};

pub const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const ANSWER_FRAMES: usize = 600;

pub struct Screen(pub Vec2);

impl Renderer for Screen {
    fn name(&self) -> &'static str {
        "screen"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.0 = Vec2::new(width as f32, height as f32);
    }

    fn physical(&self) -> Option<Vec2> {
        Some(self.0)
    }

    fn prepare(&mut self, output: &WindowOutput, _scale: f32, _background: Color32) -> bool {
        output.changed
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

pub struct Showing(pub Document);

impl App for Showing {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.show(context, rect);
    }
}

enum Window {
    Simulated(Simulation),
    Other(Box<dyn Platform>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Id(String, Option<Vec2>),
    Point(Pos2),
    Text(String),
}

impl Target {
    pub fn id(id: impl Into<String>) -> Self {
        Self::Id(id.into(), None)
    }

    pub fn point(at: Pos2) -> Self {
        Self::Point(at)
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn at(self, offset: Vec2) -> Self {
        match self {
            Self::Id(id, _) => Self::Id(id, Some(offset)),
            other => panic!("only a test id takes an offset, not {other:?}"),
        }
    }

    pub fn word(&self, pixels_per_point: f32) -> String {
        let pixels = |points: f32| points * pixels_per_point;
        match self {
            Self::Id(id, None) => format!("#{id}"),
            Self::Id(id, Some(offset)) => {
                format!("#{id}+{},{}", pixels(offset.x), pixels(offset.y))
            }
            Self::Point(at) => format!("{},{}", pixels(at.x), pixels(at.y)),
            Self::Text(text) => text.clone(),
        }
    }
}

impl From<&str> for Target {
    fn from(id: &str) -> Self {
        Self::id(id)
    }
}

impl From<&String> for Target {
    fn from(id: &String) -> Self {
        Self::id(id.as_str())
    }
}

impl From<String> for Target {
    fn from(id: String) -> Self {
        Self::id(id)
    }
}

impl From<Pos2> for Target {
    fn from(at: Pos2) -> Self {
        Self::point(at)
    }
}

type Observer = Box<dyn FnMut(&FrameOutput)>;

#[derive(Clone, Debug, PartialEq)]
pub struct HostedWindow {
    pub id: WindowId,
    pub size: Vec2,
    pub placement: HostPlacement,
}

#[derive(Default)]
pub struct Windows {
    pub open: Vec<HostedWindow>,
    pub requests: Vec<WindowRequest>,
    pub outputs: Vec<WindowOutput>,
    pub controllers: Vec<crate::runner::Controller>,
    reports: Vec<WindowReport>,
    main: Option<Vec2>,
}

impl Windows {
    pub fn window(&self, id: WindowId) -> Option<&HostedWindow> {
        self.open.iter().find(|window| window.id == id)
    }

    pub fn output(&self, id: WindowId) -> Option<&WindowOutput> {
        self.outputs.iter().find(|output| output.id == id)
    }

    fn resized(&mut self, id: WindowId, size: Vec2) {
        if let Some(window) = self.open.iter_mut().find(|window| window.id == id) {
            window.size = size;
            self.reports
                .push(WindowReport::Resized { window: id, size });
        }
    }

    fn opened(&mut self, id: WindowId, placement: &WindowPlacement, size: Vec2) {
        let placement = match placement {
            WindowPlacement::Host(placement) => placement.clone(),
            WindowPlacement::AtPointer => HostPlacement("at the pointer".to_owned()),
            WindowPlacement::Over { window, at } => {
                HostPlacement(format!("over {} at {} {}", window.raw(), at.x, at.y))
            }
        };
        self.open.retain(|window| window.id != id);
        self.open.push(HostedWindow {
            id,
            size,
            placement: placement.clone(),
        });
        self.reports
            .push(WindowReport::Resized { window: id, size });
        self.reports.push(WindowReport::Placed {
            window: id,
            placement,
        });
    }
}

struct TestHost(Rc<RefCell<Windows>>);

impl WindowHost for TestHost {
    fn input(&mut self, incoming: &mut Vec<Incoming>, view: &HostView) {
        let mut windows = self.0.borrow_mut();
        let main = view.platform / view.pixels_per_point;
        if windows.main != Some(main) {
            windows.main = Some(main);
            incoming.insert(
                0,
                Incoming::Window(WindowReport::Resized {
                    window: WindowId::MAIN,
                    size: main,
                }),
            );
        }
        incoming.retain(|item| match item {
            Incoming::Controller(controller) => {
                windows.controllers.push(*controller);
                false
            }
            _ => true,
        });
        incoming.extend(windows.reports.drain(..).map(Incoming::Window));
    }

    fn frame(&mut self, output: &FrameOutput, _view: &HostView) -> Hosted {
        let mut windows = self.0.borrow_mut();
        for request in &output.window_requests {
            windows.requests.push(request.clone());
            match request {
                WindowRequest::Open {
                    window,
                    placement,
                    size,
                } => windows.opened(*window, placement, *size),
                WindowRequest::Close(window) => {
                    windows.open.retain(|open| open.id != *window);
                    windows.outputs.retain(|output| output.id != *window);
                }
                WindowRequest::Resize { window, size } => windows.resized(*window, *size),
                WindowRequest::BeginMove(_) | WindowRequest::BeginResize { .. } => {}
            }
        }
        for window in output.windows.iter().filter(|window| !window.id.is_main()) {
            windows.outputs.retain(|output| output.id != window.id);
            windows.outputs.push(window.clone());
        }
        Hosted {
            window: output.main().clone(),
            repaint_after: Duration::MAX,
        }
    }
}

pub struct Stepped<A: App = Showing> {
    app: PhantomData<A>,
    runner: Runner,
    windows: Option<Rc<RefCell<Windows>>>,
    window: Window,
    size: Vec2,
    painted: bool,
    settling: bool,
    observers: Vec<Observer>,
    inbox: Inbox,
    steps: Vec<String>,
    closed: bool,
}

impl<A: App + 'static> Stepped<A> {
    pub fn new(context: Context, app: A, window: WindowSize, screen: Option<Vec2>) -> Self {
        Self::start(context, app, window.size, window.scale, None, |runner| {
            Window::Simulated(Simulation::new(window, screen, runner.reports().clone()))
        })
    }

    pub fn hosting_windows(context: Context, app: A, window: WindowSize) -> Self {
        let windows = Rc::new(RefCell::new(Windows::default()));
        Self::start(
            context,
            app,
            window.size,
            window.scale,
            Some(windows),
            |runner| Window::Simulated(Simulation::new(window, None, runner.reports().clone())),
        )
    }

    pub fn on(context: Context, app: A, size: Vec2, platform: impl Platform + 'static) -> Self {
        Self::start(context, app, size, 1.0, None, |_| {
            Window::Other(Box::new(platform))
        })
    }

    fn start(
        context: Context,
        app: A,
        size: Vec2,
        scale: f32,
        windows: Option<Rc<RefCell<Windows>>>,
        window: impl FnOnce(&Runner) -> Window,
    ) -> Self {
        let inbox = Inbox::default();
        let host = windows
            .clone()
            .map(|windows| Box::new(TestHost(windows)) as Box<dyn WindowHost>);
        let mut runner = Runner::new(Launch {
            options: RunOptions {
                automation: Some(inbox.clone()),
                ..RunOptions::default()
            },
            context,
            app: Box::new(app),
            host,
        });
        let window = window(&runner);
        runner
            .start(
                vec![Loaded {
                    renderer: Box::new(Screen(size * scale)),
                    fonts: None,
                }],
                Setup::new(Waker::new(|| {})),
            )
            .expect("a screen without a gpu always loads");
        runner.context().stop_clock();
        Self {
            app: PhantomData,
            runner,
            windows,
            window,
            size,
            painted: false,
            settling: true,
            observers: Vec::new(),
            inbox,
            steps: Vec::new(),
            closed: false,
        }
    }

    pub fn app(&self) -> &A {
        let app: &dyn Any = self.runner.app();
        app.downcast_ref().expect("the app is there until it exits")
    }

    pub fn app_mut(&mut self) -> &mut A {
        let app: &mut dyn Any = self.runner.app_mut();
        app.downcast_mut().expect("the app is there until it exits")
    }

    pub fn context(&self) -> &Context {
        self.runner.context()
    }

    pub fn runner(&mut self) -> &mut Runner {
        &mut self.runner
    }

    pub fn simulation(&mut self) -> Option<&mut Simulation> {
        match &mut self.window {
            Window::Simulated(simulation) => Some(simulation),
            Window::Other(_) => None,
        }
    }

    pub fn output(&self) -> Option<&FrameOutput> {
        self.runner.output()
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    pub fn frame(&mut self, inputs: Vec<Input>) -> FrameOutput {
        for input in inputs {
            self.runner.push(Incoming::points(input));
        }
        self.paint();
        self.runner
            .take_output()
            .expect("a started runner that has not exited draws every frame")
    }

    pub fn frame_in(&mut self, window: WindowId, inputs: Vec<Input>) -> FrameOutput {
        for input in inputs {
            self.runner.push(Incoming::points(input).in_window(window));
        }
        self.paint();
        self.runner
            .take_output()
            .expect("a started runner that has not exited draws every frame")
    }

    pub fn windows(&self) -> std::cell::Ref<'_, Windows> {
        self.windows
            .as_ref()
            .expect("only a harness made with hosting_windows hosts windows")
            .borrow()
    }

    pub fn take_window_requests(&mut self) -> Vec<WindowRequest> {
        std::mem::take(
            &mut self
                .windows
                .as_ref()
                .expect("only a harness made with hosting_windows hosts windows")
                .borrow_mut()
                .requests,
        )
    }

    pub fn report_window(&mut self, report: WindowReport) {
        let windows = self
            .windows
            .as_ref()
            .expect("only a harness made with hosting_windows hosts windows");
        let mut windows = windows.borrow_mut();
        match &report {
            WindowReport::Resized { window, size } => {
                if let Some(open) = windows.open.iter_mut().find(|open| open.id == *window) {
                    open.size = *size;
                }
            }
            WindowReport::Placed { window, placement } => {
                if let Some(open) = windows.open.iter_mut().find(|open| open.id == *window) {
                    open.placement = placement.clone();
                }
            }
            WindowReport::Rastered { .. } | WindowReport::Focused { .. } => {}
            WindowReport::Closed(window) => {
                windows.open.retain(|open| open.id != *window);
                windows.outputs.retain(|output| output.id != *window);
            }
        }
        windows.reports.push(report);
    }

    pub fn observe(&mut self, observer: impl FnMut(&FrameOutput) + 'static) {
        self.observers.push(Box::new(observer));
    }

    pub fn advance(&mut self, by: Duration) {
        self.runner.context().advance_clock(by);
    }

    pub fn size(&self) -> Vec2 {
        match &self.window {
            Window::Simulated(simulation) => simulation.shown(),
            Window::Other(_) => self.size,
        }
    }

    pub fn take_output(&mut self) -> FrameOutput {
        self.runner
            .take_output()
            .expect("a frame was painted since the output was last taken")
    }

    pub fn resize(&mut self, size: Vec2) {
        let scale = self.scale();
        self.resize_window(WindowSize { size, scale });
    }

    pub fn rescale(&mut self, scale: f32) {
        let size = self.size();
        self.resize_window(WindowSize { size, scale });
    }

    pub fn scale(&self) -> f32 {
        match &self.window {
            Window::Simulated(simulation) => simulation.window.scale,
            Window::Other(_) => 1.0,
        }
    }

    fn resize_window(&mut self, window: WindowSize) {
        let resized = match &mut self.window {
            Window::Simulated(simulation) => simulation.resize(Resize::Window(window)),
            Window::Other(_) => Err("only a simulated window resizes".to_owned()),
        };
        resized.unwrap_or_else(|error| panic!("{error}"));
    }

    fn paint(&mut self) {
        self.painted = true;
        self.runner.context().advance_clock(FRAME_INTERVAL);
        let platform: &mut dyn Platform = match &mut self.window {
            Window::Simulated(simulation) => simulation,
            Window::Other(platform) => &mut **platform,
        };
        self.runner.frame(platform, SafeArea::default());
        if let Some(output) = self.runner.output() {
            for observer in &mut self.observers {
                observer(output);
            }
        }
        self.closed |= self.runner.exited();
        if let Some(failure) = self.runner.failure() {
            panic!("the app panicked: {failure}\n{}", self.replay());
        }
    }

    pub fn ask(&mut self, words: &[&str]) -> Result<String, String> {
        self.steps.push(
            std::iter::once("drive")
                .chain(words.iter().copied())
                .map(quoted)
                .collect::<Vec<_>>()
                .join(" "),
        );
        if !self.painted {
            self.paint();
        }
        let owned = words.iter().map(|word| (*word).to_owned()).collect();
        let mut answered = pin!(ask(&self.inbox, owned));
        let mut waiting = TaskContext::from_waker(TaskWaker::noop());
        for _ in 0..ANSWER_FRAMES {
            self.paint();
            if let Poll::Ready(reply) = answered.as_mut().poll(&mut waiting) {
                return match reply {
                    Reply::Text(text) => Ok(text),
                    Reply::Error(error) => Err(error),
                    Reply::Image { .. } => Err("the answer is an image".to_owned()),
                    Reply::Painting(_) => Err("the answer is a painting".to_owned()),
                };
            }
        }
        panic!(
            "the app never answered {words:?} in {ANSWER_FRAMES} frames\n{}",
            self.replay()
        );
    }

    pub fn settle_after_gestures(&mut self, settling: bool) {
        self.settling = settling;
    }

    pub fn run(&mut self, words: &[&str]) -> String {
        self.ask(words)
            .unwrap_or_else(|error| panic!("{error}\n{}", self.replay()))
    }

    pub fn click(&mut self, target: impl Into<Target>) -> String {
        self.gesture(&["click"], &[target.into()], &[])
    }

    pub fn click_with(&mut self, target: impl Into<Target>, button: PointerButton) -> String {
        let button = match button {
            PointerButton::Primary => None,
            PointerButton::Secondary => Some("right"),
            PointerButton::Middle => Some("middle"),
            other => panic!("drive clicks the primary, right and middle buttons, not {other:?}"),
        };
        self.gesture(&["click"], &[target.into()], button.as_slice())
    }

    pub fn double_click(&mut self, target: impl Into<Target>) -> String {
        self.gesture(&["click"], &[target.into()], &["double"])
    }

    pub fn hover(&mut self, target: impl Into<Target>) -> String {
        self.gesture(&["move"], &[target.into()], &[])
    }

    pub fn leave(&mut self) -> String {
        self.gesture(&["leave"], &[], &[])
    }

    pub fn down(&mut self, target: impl Into<Target>, button: PointerButton) -> String {
        let button = button_name(button);
        self.gesture(&["down"], &[target.into()], &[&button])
    }

    pub fn up(&mut self, target: impl Into<Target>, button: PointerButton) -> String {
        let button = button_name(button);
        self.gesture(&["up"], &[target.into()], &[&button])
    }

    pub fn drag(&mut self, from: impl Into<Target>, to: impl Into<Target>) -> String {
        self.gesture(&["drag"], &[from.into(), to.into()], &[])
    }

    pub fn drag_in(
        &mut self,
        from: impl Into<Target>,
        to: impl Into<Target>,
        steps: usize,
    ) -> String {
        let steps = steps.to_string();
        self.gesture(&["drag"], &[from.into(), to.into()], &[&steps])
    }

    pub fn scroll(&mut self, target: impl Into<Target>, by: Vec2) -> String {
        let scale = self.runner.context().pixels_per_point();
        let (down, right) = ((by.y * scale).to_string(), (by.x * scale).to_string());
        self.gesture(&["scroll"], &[target.into()], &[&down, &right])
    }

    pub fn motion(&mut self, by: Vec2) -> String {
        let scale = self.runner.context().pixels_per_point();
        let (right, down) = ((by.x * scale).to_string(), (by.y * scale).to_string());
        self.gesture(&["motion"], &[], &[&right, &down])
    }

    pub fn wheel(&mut self, target: impl Into<Target>, down: i32, right: i32) -> String {
        let (down, right) = (down.to_string(), right.to_string());
        self.gesture(&["wheel"], &[target.into()], &[&down, &right])
    }

    pub fn zoom(&mut self, target: impl Into<Target>, factor: f32) -> String {
        let factor = factor.to_string();
        self.gesture(&["zoom"], &[target.into()], &[&factor])
    }

    pub fn pinch(&mut self, target: impl Into<Target>, factor: f32) -> String {
        let factor = factor.to_string();
        self.gesture(&["pinch"], &[target.into()], &[&factor])
    }

    pub fn tap(&mut self, target: impl Into<Target>) -> String {
        self.gesture(&["tap"], &[target.into()], &[])
    }

    pub fn swipe(&mut self, from: impl Into<Target>, to: impl Into<Target>) -> String {
        self.gesture(&["swipe"], &[from.into(), to.into()], &[])
    }

    pub fn fingers<T: Into<Target> + Clone>(
        &mut self,
        phase: TouchPhase,
        fingers: &[(u64, T)],
    ) -> String {
        let phase = match phase {
            TouchPhase::Start => "down",
            TouchPhase::Move => "move",
            TouchPhase::End => "up",
            TouchPhase::Cancel => "cancel",
        };
        let fingers: Vec<String> = fingers
            .iter()
            .map(|(finger, target)| format!("{finger}={}", self.word(target.clone().into())))
            .collect();
        let fingers: Vec<&str> = fingers.iter().map(String::as_str).collect();
        self.gesture(&["touch", phase], &[], &fingers)
    }

    pub fn type_text(&mut self, text: &str) -> String {
        self.gesture(&["type"], &[], &[text])
    }

    pub fn key(&mut self, key: Key, modifiers: Modifiers) -> String {
        let chord = keys::chord_name(key, modifiers)
            .unwrap_or_else(|| panic!("drive has no name for {key:?}"));
        self.gesture(&["key"], &[], &[&chord])
    }

    pub fn key_down(&mut self, key: Key) -> String {
        let name = key
            .name()
            .unwrap_or_else(|| panic!("drive has no name for {key:?}"));
        self.gesture(&["keydown"], &[], &[&name])
    }

    pub fn key_up(&mut self, key: Key) -> String {
        let name = key
            .name()
            .unwrap_or_else(|| panic!("drive has no name for {key:?}"));
        self.gesture(&["keyup"], &[], &[&name])
    }

    pub fn wait(&mut self, target: impl Into<Target>) -> String {
        self.gesture(&["wait"], &[target.into()], &[])
    }

    pub fn gone(&mut self, target: impl Into<Target>) -> String {
        self.gesture(&["gone"], &[target.into()], &[])
    }

    pub fn pause(&mut self, by: Duration) -> String {
        self.run(&["pause", &by.as_millis().to_string()])
    }

    pub fn settle(&mut self) -> String {
        self.run(&["settle"])
    }

    fn gesture(&mut self, command: &[&str], targets: &[Target], rest: &[&str]) -> String {
        let targets: Vec<String> = targets
            .iter()
            .map(|target| self.word(target.clone()))
            .collect();
        let mut words = match self.settling {
            true => Vec::new(),
            false => vec!["--no-settle"],
        };
        words.extend(command);
        words.extend(targets.iter().map(String::as_str));
        words.extend(rest);
        self.run(&words)
    }

    fn word(&self, target: Target) -> String {
        target.word(self.runner.context().pixels_per_point())
    }

    fn replay(&self) -> String {
        let mut replay = String::from("the steps so far, as drive commands:\n");
        for step in &self.steps {
            replay.push_str("  ");
            replay.push_str(step);
            replay.push('\n');
        }
        replay
    }
}

impl Stepped<Showing> {
    pub fn document(&self) -> &Document {
        &self.app().0
    }

    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.app_mut().0
    }
}

pub fn button_name(button: PointerButton) -> String {
    match button {
        PointerButton::Primary => "left".to_owned(),
        PointerButton::Secondary => "right".to_owned(),
        PointerButton::Middle => "middle".to_owned(),
        PointerButton::Back => "back".to_owned(),
        PointerButton::Forward => "forward".to_owned(),
        PointerButton::Other(code) => format!("button:{code:#x}"),
    }
}

fn quoted(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "#+,.-_:/@=".contains(c));
    match plain {
        true => word.to_owned(),
        false => format!("'{}'", word.replace('\'', "'\\''")),
    }
}
