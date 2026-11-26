use std::any::Any;
use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};
use std::time::Duration;

use beui_core::context::RendererInfo;
use beui_core::context::{Context, EscapedOutput, FrameOutput, WindowOutput};
use beui_core::document::Document;
pub use beui_core::document::delay_measurement;
use beui_core::program::{Program, SafeArea, Setup, Waker};
use beui_core::window::{Resize, WindowSize};
use beui_layer_drive::{Drive, Inbox, Reply, ask};
use beui_runner::renderer::{Loaded, Renderer};
use beui_runner::{
    Incoming, Launch, Layer, LayerFrame, PlatformServices, RunOptions, Runner, Shown,
};
use beui_types::Color32;
use beui_types::{HostPlacement, WindowId, WindowPlacement, WindowReport, WindowRequest};
use beui_types::{Input, PointerButton};
use beui_types::{Pos2, Rect, Vec2};

use crate::Simulation;
use beui_core::program::accessibility_dump::Rendered;

mod find;
mod gui;

pub use find::{Match, find_text};
pub use gui::{Gui, Recording, Snapshots, Wakes, Wraps};

pub const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const ANSWER_FRAMES: usize = 600;

pub struct Screen(pub Vec2);

enum Hosting {
    Alone,
    Tested(Rc<RefCell<Windows>>),
    By(Box<dyn Layer>, Setup),
}

impl Renderer for Screen {
    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.0 = Vec2::new(width as f32, height as f32);
    }

    fn physical(&self) -> Option<Vec2> {
        (self.0.x > 0.0 && self.0.y > 0.0).then_some(self.0)
    }

    fn prepare(&mut self, output: &WindowOutput, _scale: f32, _background: Color32) -> bool {
        output.changed
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

pub struct Showing(pub Document);

impl Program for Showing {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.show(context, rect);
    }
}

enum Window {
    Simulated(Simulation),
    Other(Box<dyn PlatformServices>),
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
    pub controllers: Vec<beui_runner::Controller>,
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

impl Layer for TestHost {
    fn attach(&mut self, context: &Context) {
        context.set_window_hosting(beui_core::window::WindowHosting::Any);
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, frame: &mut LayerFrame<'_>) {
        let mut windows = self.0.borrow_mut();
        let main = frame.view.platform / frame.view.pixels_per_point;
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

    fn output(&mut self, shown: &mut Shown, _output: &FrameOutput, _frame: &mut LayerFrame<'_>) {
        let mut windows = self.0.borrow_mut();
        for request in std::mem::take(&mut shown.requests).iter() {
            windows.requests.push(request.clone());
            match request {
                WindowRequest::Open {
                    window,
                    placement,
                    size,
                    ..
                } => windows.opened(*window, placement, *size),
                WindowRequest::Close(window) => {
                    windows.open.retain(|open| open.id != *window);
                    windows.outputs.retain(|output| output.id != *window);
                }
                WindowRequest::Resize { window, size } => windows.resized(*window, *size),
                WindowRequest::BeginMove(_)
                | WindowRequest::BeginResize { .. }
                | WindowRequest::Raise(_)
                | WindowRequest::Place { .. } => {}
            }
        }
        for window in shown.windows.iter().filter(|window| !window.id.is_main()) {
            windows.outputs.retain(|output| output.id != window.id);
            windows.outputs.push(window.clone());
        }
        shown.windows.retain(|window| window.id.is_main());
    }
}

pub struct Stepped<A: Program = Showing> {
    program: PhantomData<A>,
    runner: Runner,
    windows: Option<Rc<RefCell<Windows>>>,
    window: Window,
    size: Vec2,
    painted: bool,
    settling: bool,
    inbox: Inbox,
    steps: Vec<String>,
    closed: bool,
    latest: Option<Latest>,
    seen: Seen,
    wakes: Arc<Wakes>,
    recording: Recording,
}

/// What the last frame painted said, kept from every frame however it was painted.
#[derive(Default)]
struct Seen {
    tree: Rc<Rendered>,
    changed: bool,
    repaint: bool,
    repaint_after: Duration,
}

struct Latest {
    main: WindowOutput,
    escaped: Vec<EscapedOutput>,
    pixels_per_point: f32,
}

impl Latest {
    fn of(output: &FrameOutput) -> Self {
        Self {
            main: output.main().clone(),
            escaped: output.escaped.clone(),
            pixels_per_point: output.pixels_per_point(),
        }
    }
}

impl<A: Program + 'static> Stepped<A> {
    pub fn new(context: Context, program: A, window: WindowSize, screen: Option<Vec2>) -> Self {
        Self::start(
            context,
            program,
            window.size,
            window.scale,
            Hosting::Alone,
            |runner| Window::Simulated(Simulation::new(window, screen, runner.reports().clone())),
        )
    }

    pub fn hosting_windows(context: Context, program: A, window: WindowSize) -> Self {
        let windows = Rc::new(RefCell::new(Windows::default()));
        Self::start(
            context,
            program,
            window.size,
            window.scale,
            Hosting::Tested(windows),
            |runner| Window::Simulated(Simulation::new(window, None, runner.reports().clone())),
        )
    }

    pub fn hosted(
        context: Context,
        program: A,
        window: WindowSize,
        host: Box<dyn Layer>,
        setup: Setup,
    ) -> Self {
        Self::start(
            context,
            program,
            window.size,
            window.scale,
            Hosting::By(host, setup),
            |runner| Window::Simulated(Simulation::new(window, None, runner.reports().clone())),
        )
    }

    pub fn on(
        context: Context,
        program: A,
        size: Vec2,
        platform: impl PlatformServices + 'static,
    ) -> Self {
        Self::start(context, program, size, 1.0, Hosting::Alone, |_| {
            Window::Other(Box::new(platform))
        })
    }

    fn start(
        context: Context,
        program: A,
        size: Vec2,
        scale: f32,
        hosting: Hosting,
        window: impl FnOnce(&Runner) -> Window,
    ) -> Self {
        let inbox = Inbox::default();
        let wakes = Arc::new(Wakes::default());
        let woken = Arc::clone(&wakes);
        let waker = move |chained: Option<Waker>| {
            Waker::new(move || {
                if let Some(chained) = &chained {
                    chained.wake();
                }
                woken.wake();
            })
        };
        let (windows, host, setup) = match hosting {
            Hosting::Alone => (None, None, Setup::new(waker(None))),
            Hosting::Tested(windows) => (
                Some(windows.clone()),
                Some(Box::new(TestHost(windows)) as Box<dyn Layer>),
                Setup::new(waker(None)),
            ),
            Hosting::By(host, mut setup) => {
                setup.waker = waker(Some(setup.waker.clone()));
                (None, Some(host), setup)
            }
        };
        let layers = std::iter::once(Box::new(Drive::new(inbox.clone())) as Box<dyn Layer>)
            .chain(host)
            .collect();
        let mut runner = Runner::new(Launch {
            options: RunOptions::default(),
            context,
            program: Box::new(program),
            layers,
        });
        let window = window(&runner);
        runner
            .start(
                Loaded {
                    renderer: Box::new(Screen(size * scale)),
                    fonts: None,
                },
                setup,
            )
            .expect("a screen without a gpu always loads");
        runner.context().stop_clock();
        Self {
            program: PhantomData,
            runner,
            windows,
            window,
            size,
            painted: false,
            settling: true,
            inbox,
            steps: Vec::new(),
            closed: false,
            latest: None,
            seen: Seen::default(),
            wakes,
            recording: Recording::default(),
        }
    }

    pub fn program(&self) -> &A {
        let program: &dyn Any = self.runner.program();
        program
            .downcast_ref()
            .expect("the program is there until it exits")
    }

    pub fn program_mut(&mut self) -> &mut A {
        let program: &mut dyn Any = self.runner.program_mut();
        program
            .downcast_mut()
            .expect("the program is there until it exits")
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

    pub fn frame(&mut self, inputs: Vec<Input>) -> FrameOutput {
        for input in inputs {
            self.runner.push(Incoming::points(input));
        }
        self.paint();
        let output = self
            .runner
            .take_output()
            .expect("a started runner that has not exited draws every frame");
        self.latest = Some(Latest::of(&output));
        output
    }

    pub fn frame_in(&mut self, window: WindowId, inputs: Vec<Input>) -> FrameOutput {
        for input in inputs {
            self.runner.push(Incoming::points(input).in_window(window));
        }
        self.paint();
        let output = self
            .runner
            .take_output()
            .expect("a started runner that has not exited draws every frame");
        self.latest = Some(Latest::of(&output));
        output
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
            WindowReport::Rastered { .. }
            | WindowReport::Moved { .. }
            | WindowReport::Focused { .. } => {}
            WindowReport::Closed(window) => {
                windows.open.retain(|open| open.id != *window);
                windows.outputs.retain(|output| output.id != *window);
            }
        }
        windows.reports.push(report);
    }

    /// The window's size in points, which `Gui::size` answers for a document.
    fn shown_size(&self) -> Vec2 {
        match &self.window {
            Window::Simulated(simulation) => simulation.shown(),
            Window::Other(_) => self.size,
        }
    }

    pub fn take_output(&mut self) -> FrameOutput {
        let output = self
            .runner
            .take_output()
            .expect("a frame was painted since the output was last taken");
        self.latest = Some(Latest::of(&output));
        output
    }

    pub fn resize(&mut self, size: Vec2) {
        let scale = self.scale();
        self.resize_window(WindowSize { size, scale });
    }

    pub fn rescale(&mut self, scale: f32) {
        let size = self.shown_size();
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
        let platform: &mut dyn PlatformServices = match &mut self.window {
            Window::Simulated(simulation) => simulation,
            Window::Other(platform) => &mut **platform,
        };
        self.runner.frame(platform, SafeArea::default());
        if let Some(output) = self.runner.output() {
            self.seen = Seen {
                tree: output.tree(),
                changed: output.main().changed,
                repaint: output.repaint,
                repaint_after: output.repaint_after,
            };
        }
        self.closed |= self.runner.exited();
        if let Some(failure) = self.runner.failure() {
            panic!(
                "the program panicked: {failure}\n{}",
                gui::replay(&self.steps)
            );
        }
    }

    pub fn ask(&mut self, words: &[&str]) -> Result<String, String> {
        match self.reply(words) {
            Reply::Text(text) => Ok(text),
            Reply::Error(error) => Err(error),
            Reply::Image { .. } => Err("the answer is an image".to_owned()),
            Reply::Painting(_) => Err("the answer is a painting".to_owned()),
        }
    }

    pub fn painting(&mut self) -> beui_core::painting::Painting {
        self.painted_by(&["paint", "in", "platform"])
    }

    pub fn painting_of(&mut self, target: &str) -> beui_core::painting::Painting {
        self.painted_by(&["paint", target])
    }

    fn painted_by(&mut self, words: &[&str]) -> beui_core::painting::Painting {
        let painted = match self.reply(words) {
            Reply::Painting(bytes) => beui_core::painting::Painting::decode(&bytes),
            Reply::Error(error) => Err(error),
            Reply::Text(_) | Reply::Image { .. } => Err("the answer is not a painting".to_owned()),
        };
        painted.unwrap_or_else(|error| panic!("{error}\n{}", gui::replay(&self.steps)))
    }

    fn reply(&mut self, words: &[&str]) -> Reply {
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
                return reply;
            }
        }
        panic!(
            "the program never answered {words:?} in {ANSWER_FRAMES} frames\n{}",
            gui::replay(&self.steps)
        );
    }

    /// Whether a gesture waits for the program to settle, as `drive` does, or answers after
    /// the frame that took it (`--no-settle`).
    pub fn settle_after_gestures(&mut self, settling: bool) {
        self.settling = settling;
    }

    /// Runs one `drive` command and answers what it answered.
    pub fn drive(&mut self, words: &[&str]) -> String {
        self.ask(words)
            .unwrap_or_else(|error| panic!("{error}\n{}", gui::replay(&self.steps)))
    }

    /// Waits until the tree shows `target` (`drive wait`).
    pub fn wait(&mut self, target: impl Into<Target>) -> String {
        let target = self.word(target.into());
        self.drive(&["wait", &target])
    }

    /// Waits until the tree no longer shows `target` (`drive wait not`).
    pub fn gone(&mut self, target: impl Into<Target>) -> String {
        let target = self.word(target.into());
        self.drive(&["wait", "not", &target])
    }

    fn word(&self, target: Target) -> String {
        target.word(self.runner.context().pixels_per_point())
    }
}

impl Gui for Stepped<Showing> {
    type DocumentRef<'a> = &'a Document;
    type DocumentMut<'a> = &'a mut Document;

    fn document(&self) -> &Document {
        &self.program().0
    }

    fn document_mut(&mut self) -> &mut Document {
        &mut self.program_mut().0
    }

    fn tree(&self) -> Rc<Rendered> {
        Rc::clone(&self.seen.tree)
    }

    fn size(&self) -> Vec2 {
        self.shown_size()
    }

    fn pixels_per_point(&self) -> f32 {
        self.runner.context().pixels_per_point()
    }

    /// Sends the gesture to the drive layer, which finds and reveals its targets.
    fn gesture(&mut self, command: &str, words: &[String], _targets: &[Target]) {
        let mut all = match self.settling {
            true => Vec::new(),
            false => vec!["--no-settle"],
        };
        all.push(command);
        all.extend(words.iter().map(String::as_str));
        self.drive(&all);
    }

    fn run(&mut self) {
        self.paint();
    }

    fn settled(&self) -> bool {
        !self.seen.repaint && self.seen.repaint_after > Duration::ZERO
    }

    fn last_frame(&self) -> (bool, Duration) {
        (self.seen.changed, self.seen.repaint_after)
    }

    fn advance(&mut self, by: Duration) {
        self.runner.context().advance_clock(by);
    }

    /// As `drive pause`: a frame at a time, each only once the program has nothing to
    /// wait for.
    fn pause(&mut self, by: Duration) {
        self.drive(&["pause", &by.as_millis().to_string()]);
    }

    fn wakes(&self) -> &Wakes {
        &self.wakes
    }

    fn steps(&self) -> &[String] {
        &self.steps
    }

    /// Waits as `drive settle` does: with the clock stopped, until no input waits to be
    /// handled. Time passes only with `advance` or `pause`.
    fn settle(&mut self) {
        self.drive(&["settle"]);
    }
}

impl Snapshots for Stepped<Showing> {
    fn capture(&mut self) -> beui_paint_snapshot::Snapshot {
        self.painted_snapshot()
    }

    fn out_of_bounds(&self) -> Vec<String> {
        self.document()
            .overflows()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn recording(&mut self) -> &mut Recording {
        &mut self.recording
    }
}

impl Stepped<Showing> {
    fn painted_snapshot(&mut self) -> beui_paint_snapshot::Snapshot {
        let size = self.size();
        let latest = self.latest();
        let painting = painting(
            &latest.main,
            size,
            latest.pixels_per_point,
            latest.main.window.appearance.clear_color,
        );
        beui_paint_snapshot::Snapshot::of_painting(painting)
            .expect("the painting could not be encoded")
    }

    pub fn snapshot_layers(&mut self, name: &str) {
        let space = self
            .context()
            .layer_space(WindowId::MAIN)
            .expect("snapshot_layers shows the main window in the layer space it was given");
        let latest = self.latest();
        let composite = beui_core::escape::composite_layers(
            &latest.main,
            &latest.escaped,
            space,
            latest.main.window.appearance.clear_color,
        );
        let painting = painting(
            &composite,
            composite.size,
            latest.pixels_per_point,
            LAYER_HOST,
        );
        beui_paint_snapshot::assert_painting(name, painting);
    }

    pub fn snapshot_window(&mut self, name: &str, window: WindowId) {
        let output = self
            .windows()
            .output(window)
            .cloned()
            .unwrap_or_else(|| panic!("window {window:?} has painted nothing"));
        let painting = painting(
            &output,
            output.size,
            output.pixels_per_point,
            output.window.appearance.clear_color,
        );
        beui_paint_snapshot::assert_painting(name, painting);
    }

    fn latest(&mut self) -> &Latest {
        if !self.painted {
            self.paint();
        }
        if let Some(output) = self.runner.output() {
            self.latest = Some(Latest::of(output));
        }
        self.latest
            .as_ref()
            .expect("a painted runner has drawn a frame")
    }
}

const LAYER_HOST: Color32 = Color32::from_rgb(24, 26, 32);

fn painting(
    output: &WindowOutput,
    size: Vec2,
    pixels_per_point: f32,
    background: Color32,
) -> beui_core::painting::Painting {
    beui_core::painting::painting(
        output,
        size,
        pixels_per_point,
        background,
        beui_core::painting::Punches::Shown,
    )
    .unwrap_or_else(|error| panic!("the painting could not be rendered: {error}"))
}

pub fn assert_fits(name: &str, mut found: Vec<String>) {
    found.sort();
    found.dedup();
    if found.is_empty() {
        return;
    }
    panic!(
        "the painting {name} shows {} element(s) out of bounds, cut off where nothing scrolls to them, drawn outside their own box, or with an outline cut while their box shows (give a ring room: padding inside the scroll around its content, or `bleed` on a scroll inside a padded surface):\n  {}",
        found.len(),
        found.join("\n  ")
    );
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
