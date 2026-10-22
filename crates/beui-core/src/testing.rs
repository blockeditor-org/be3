use std::cell::{Ref, RefCell, RefMut};
use std::future::Future;
use std::pin::pin;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};
use std::time::Duration;

use crate::app::automation::{Inbox, Reply, Simulation, WindowSize, ask};
use crate::app::{App, SafeArea, Setup, Waker};
use crate::color::Color32;
use crate::context::RendererInfo;
use crate::context::{Context, FrameOutput};
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::renderer::{Loaded, Renderer};
use crate::runner::{Launch, Platform, RunOptions, Runner};

const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
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

    fn prepare(&mut self, output: &FrameOutput, _scale: f32, _background: Color32) -> bool {
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

struct Shared<A>(Rc<RefCell<A>>);

impl<A: App> App for Shared<A> {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.borrow_mut().update(context, rect);
    }

    fn setup(&mut self, setup: &Setup) {
        self.0.borrow_mut().setup(setup);
    }

    fn renderer_replaced(&mut self, setup: &Setup) {
        self.0.borrow_mut().renderer_replaced(setup);
    }

    fn close_requested(&mut self) -> bool {
        self.0.borrow_mut().close_requested()
    }

    fn exiting(&mut self) {
        self.0.borrow_mut().exiting();
    }

    fn busy(&self) -> bool {
        self.0.borrow().busy()
    }

    fn environment(&self) -> Vec<(String, String)> {
        self.0.borrow().environment()
    }

    fn drawing(&self) -> bool {
        self.0.borrow().drawing()
    }

    fn request_painting(&mut self) {
        self.0.borrow_mut().request_painting();
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

    fn word(&self, pixels_per_point: f32) -> String {
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

impl From<Pos2> for Target {
    fn from(at: Pos2) -> Self {
        Self::point(at)
    }
}

pub struct Stepped<A: App = Showing> {
    runner: Runner,
    app: Rc<RefCell<A>>,
    window: Window,
    inbox: Inbox,
    steps: Vec<String>,
    closed: bool,
}

impl<A: App + 'static> Stepped<A> {
    pub fn new(context: Context, app: A, window: WindowSize, screen: Option<Vec2>) -> Self {
        Self::start(context, app, window.size * window.scale, |runner| {
            Window::Simulated(Simulation::new(window, screen, runner.reports().clone()))
        })
    }

    pub fn on(context: Context, app: A, size: Vec2, platform: impl Platform + 'static) -> Self {
        Self::start(context, app, size, |_| Window::Other(Box::new(platform)))
    }

    fn start(
        context: Context,
        app: A,
        physical: Vec2,
        window: impl FnOnce(&Runner) -> Window,
    ) -> Self {
        let inbox = Inbox::default();
        let app = Rc::new(RefCell::new(app));
        let mut runner = Runner::new(Launch {
            options: RunOptions {
                automation: Some(inbox.clone()),
                ..RunOptions::default()
            },
            context,
            app: Box::new(Shared(Rc::clone(&app))),
        });
        let window = window(&runner);
        runner
            .start(
                vec![Loaded {
                    renderer: Box::new(Screen(physical)),
                    fonts: None,
                }],
                Setup::new(Waker::new(|| {})),
            )
            .expect("a screen without a gpu always loads");
        runner.context().stop_clock();
        let mut stepped = Self {
            runner,
            app,
            window,
            inbox,
            steps: Vec::new(),
            closed: false,
        };
        stepped.frame();
        stepped
    }

    pub fn app(&self) -> Ref<'_, A> {
        self.app.borrow()
    }

    pub fn app_mut(&self) -> RefMut<'_, A> {
        self.app.borrow_mut()
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

    pub fn frame(&mut self) {
        self.runner.context().advance_clock(FRAME_INTERVAL);
        let platform: &mut dyn Platform = match &mut self.window {
            Window::Simulated(simulation) => simulation,
            Window::Other(platform) => &mut **platform,
        };
        self.runner.frame(platform, SafeArea::default());
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
        let owned = words.iter().map(|word| (*word).to_owned()).collect();
        let mut answered = pin!(ask(&self.inbox, owned));
        let mut waiting = TaskContext::from_waker(TaskWaker::noop());
        for _ in 0..ANSWER_FRAMES {
            self.frame();
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

    pub fn run(&mut self, words: &[&str]) -> String {
        self.ask(words)
            .unwrap_or_else(|error| panic!("{error}\n{}", self.replay()))
    }

    pub fn click(&mut self, target: impl Into<Target>) -> String {
        let target = self.word(target.into());
        self.run(&["click", &target])
    }

    pub fn drag(&mut self, from: impl Into<Target>, to: impl Into<Target>) -> String {
        let (from, to) = (self.word(from.into()), self.word(to.into()));
        self.run(&["drag", &from, &to])
    }

    pub fn type_text(&mut self, text: &str) -> String {
        self.run(&["type", text])
    }

    pub fn key(&mut self, chord: &str) -> String {
        self.run(&["key", chord])
    }

    pub fn wait(&mut self, target: impl Into<Target>) -> String {
        let target = self.word(target.into());
        self.run(&["wait", &target])
    }

    pub fn settle(&mut self) -> String {
        self.run(&["settle"])
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
    pub fn document(&self) -> Ref<'_, Document> {
        Ref::map(self.app(), |showing| &showing.0)
    }

    pub fn document_mut(&self) -> RefMut<'_, Document> {
        RefMut::map(self.app_mut(), |showing| &mut showing.0)
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
