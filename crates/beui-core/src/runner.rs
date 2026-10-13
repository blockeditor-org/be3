use std::any::Any;
use std::error::Error;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use accesskit::TreeUpdate;

use crate::app::accessibility_dump::AccessibilityDump;
use crate::app::automation::{Automation, Inbox, Settled, Simulation, View, World};
use crate::app::{App, SafeArea, Setup, Waker, next_batch, safe_rect};
use crate::clipboard::{PasteAnswer, PasteKind, PasteRequest, Pasted};
use crate::context::{ActionGroup, Context, FrameOutput};
use crate::file_picker::{FilePick, FilePickAnswer, FilePickId, FilePickRequest};
use crate::geometry::Vec2;
use crate::input::{Event, RawInput};
use crate::renderer::{Loaded, Renderers};
use crate::window::{WindowReports, WindowState};

mod intake;

use intake::{Drain, Intake};
pub use intake::{Input, Keystroke, SCROLL_LINE, SCROLL_PAGE, ScrollDelta};

type Answered = (FilePickId, FilePick);

pub struct RunOptions {
    pub title: String,
    pub app_id: Option<String>,
    pub size: Vec2,
    pub accessibility_tree: bool,
    pub automation: Option<Inbox>,
}

impl RunOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            app_id: None,
            size: Vec2::new(1280.0, 800.0),
            accessibility_tree: false,
            automation: None,
        }
    }
}

pub struct Launch {
    pub options: RunOptions,
    pub context: Context,
    pub app: Box<dyn App>,
}

pub type Running = Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>;

pub trait Adapter {
    fn name(&self) -> &'static str;

    fn run(self: Box<Self>, launch: Launch) -> Running;
}

pub trait Platform {
    fn copy(&mut self, _text: String) {}

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        let _ = kinds;
        answer.send(None);
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        let _ = request;
        answer.send(Err("Choosing a file is unavailable here".to_owned()));
    }

    fn delivers_paste_shortcut(&self) -> bool {
        false
    }

    fn show_window(&mut self, _window: &WindowState, _shown: &WindowState) {}

    fn simulation(&mut self) -> Option<&mut Simulation> {
        None
    }

    fn set_fullscreen(&mut self, _fullscreen: bool) {}

    fn publish_accessibility(&mut self, _tree: &mut dyn FnMut() -> TreeUpdate) {}
}

pub struct Frame {
    pub pending: bool,
    pub deferred: bool,
    pub repaint: bool,
    pub repaint_after: Duration,
}

impl Frame {
    pub fn again(&self) -> bool {
        self.deferred || self.repaint || self.repaint_after.is_zero()
    }

    pub fn wait(&self) -> Option<Duration> {
        match self.again() {
            true => Some(Duration::ZERO),
            false => (self.repaint_after != Duration::MAX).then_some(self.repaint_after),
        }
    }

    pub fn next(&self, presenting: bool) -> Option<Duration> {
        match presenting {
            true => Some(Duration::ZERO),
            false => self.wait(),
        }
    }
}

struct Exited;

impl App for Exited {
    fn update(&mut self, _context: &Context, _rect: crate::geometry::Rect) {}
}

pub struct Runner {
    app: Box<dyn App>,
    context: Context,
    title: String,
    renderers: Option<Renderers>,
    waker: Option<Waker>,
    events: Vec<Event>,
    intake: Intake,
    picks: (Sender<Answered>, Receiver<Answered>),
    pastes: (Sender<Pasted>, Receiver<Pasted>),
    accessibility: Option<AccessibilityDump>,
    publishing_accessibility: bool,
    automation: Option<Automation>,
    actions: Vec<ActionGroup>,
    test_ids: std::collections::HashMap<String, crate::geometry::Rect>,
    unpresented: bool,
    closing: bool,
    asked: WindowState,
    shown: WindowState,
    reports: WindowReports,
    output: Option<FrameOutput>,
    reports_screens: bool,
    exited: bool,
    failure: Option<String>,
}

impl Runner {
    pub fn new(launch: Launch) -> Self {
        let Launch {
            options,
            context,
            app,
        } = launch;
        let automation = options.automation.map(Automation::new);
        context.set_test_ids_published(automation.is_some());
        if automation.is_some() {
            context.set_automated(true);
        }
        let accessibility =
            (options.accessibility_tree || automation.is_some()).then(AccessibilityDump::new);
        Self {
            app,
            context,
            title: options.title,
            renderers: None,
            waker: None,
            events: Vec::new(),
            intake: Intake::default(),
            picks: mpsc::channel(),
            pastes: mpsc::channel(),
            accessibility,
            publishing_accessibility: false,
            automation,
            actions: Vec::new(),
            test_ids: std::collections::HashMap::new(),
            unpresented: false,
            closing: false,
            asked: WindowState::default(),
            shown: WindowState::default(),
            reports: WindowReports::default(),
            output: None,
            reports_screens: false,
            exited: false,
            failure: None,
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn start(&mut self, loaded: Vec<Loaded>, mut setup: Setup) -> Result<(), Box<dyn Error>> {
        let renderers = Renderers::new(&self.context, loaded)?;
        renderers.provide(&mut setup);
        self.renderers = Some(renderers);
        self.waker = Some(setup.waker.clone());
        if let Some(automation) = &self.automation {
            automation.inbox().set_waker(setup.waker.clone());
        }
        self.app.setup(&setup);
        Ok(())
    }

    pub fn started(&self) -> bool {
        self.renderers.is_some()
    }

    pub fn renderers(&mut self) -> Option<&mut Renderers> {
        self.renderers.as_mut()
    }

    pub fn push(&mut self, input: impl Into<Input>) {
        self.intake.incoming.push(input.into());
    }

    pub fn pointer_button(
        &mut self,
        pos: crate::geometry::Pos2,
        button: crate::input::PointerButton,
        pressed: bool,
    ) {
        self.push(Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: crate::input::Modifiers::NONE,
        });
    }

    pub fn modifiers(&self) -> crate::input::Modifiers {
        self.intake.modifiers()
    }

    pub fn has_events(&self) -> bool {
        !self.events.is_empty() || !self.intake.incoming.is_empty() || self.reports.pending()
    }

    pub fn reports(&self) -> WindowReports {
        self.reports.clone()
    }

    pub fn fullscreen_changed(&mut self, fullscreen: bool) {
        self.reports.fullscreen(fullscreen);
    }

    pub fn window(&self) -> &WindowState {
        &self.shown
    }

    pub fn take_output(&mut self) -> Option<FrameOutput> {
        self.output.take()
    }

    pub fn keep_accessibility(&mut self) {
        if self.accessibility.is_none() {
            self.accessibility = Some(AccessibilityDump::new());
            self.context.reset_accessibility();
        }
    }

    pub fn accessibility_requested(&mut self) {
        self.publishing_accessibility = true;
        self.context.reset_accessibility();
    }

    pub fn accessibility_deactivated(&mut self) {
        self.publishing_accessibility = false;
    }

    pub fn accessibility_action(&mut self, request: accesskit::ActionRequest) {
        self.context.accessibility_action(request);
    }

    pub fn accessibility(&self) -> Option<&AccessibilityDump> {
        self.accessibility.as_ref()
    }

    pub fn output(&self) -> Option<&FrameOutput> {
        self.output.as_ref()
    }

    pub fn accessibility_text(&self) -> Option<&str> {
        self.accessibility.as_ref().map(AccessibilityDump::text)
    }

    pub fn os_close(&mut self) -> bool {
        let accepted = self
            .contain(|runner| runner.app.close_requested())
            .unwrap_or(true);
        if accepted {
            self.exit();
        }
        accepted
    }

    pub fn exit(&mut self) {
        if !self.exited {
            self.exited = true;
            let app = &mut self.app;
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| app.exiting())) {
                self.failure
                    .get_or_insert_with(|| format!("while exiting: {}", panic_message(&*payload)));
            }
            self.app = Box::new(Exited);
        }
    }

    pub fn exited(&self) -> bool {
        self.exited
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub fn finish(mut self) -> Result<(), Box<dyn Error>> {
        self.exit();
        match self.failure.take() {
            Some(failure) => Err(format!("the app stopped after a panic: {failure}").into()),
            None => Ok(()),
        }
    }

    pub fn frame(
        &mut self,
        platform: &mut dyn Platform,
        pixels_per_point: f32,
        safe_area: SafeArea,
    ) -> Option<Frame> {
        if self.exited {
            return None;
        }
        self.contain(|runner| runner.run_frame(platform, pixels_per_point, safe_area))
            .flatten()
    }

    fn contain<R>(&mut self, act: impl FnOnce(&mut Self) -> R) -> Option<R> {
        match catch_unwind(AssertUnwindSafe(|| act(self))) {
            Ok(done) => Some(done),
            Err(payload) => {
                self.failure.get_or_insert_with(|| panic_message(&*payload));
                self.exit();
                None
            }
        }
    }

    fn run_frame(
        &mut self,
        platform: &mut dyn Platform,
        pixels_per_point: f32,
        safe_area: SafeArea,
    ) -> Option<Frame> {
        let renderers = self.renderers.as_mut()?;
        match renderers.recover() {
            Ok(false) => {}
            Ok(true) => {
                if let Some(waker) = &self.waker {
                    let mut setup = Setup::new(waker.clone());
                    renderers.provide(&mut setup);
                    self.app.renderer_replaced(&setup);
                }
            }
            Err(error) => panic!("beui: the GPU was lost and could not be reopened: {error}"),
        }
        while let Ok(pasted) = self.pastes.1.try_recv() {
            self.events.push(Event::Paste(pasted));
        }
        if let Err(error) = renderers.follow_choice(&self.context) {
            eprintln!("beui: could not switch renderers: {error}");
        }
        if let Some(automation) = &mut self.automation
            && let Some(dump) = &self.accessibility
        {
            let view = View {
                lines: dump.lines(),
                tree: dump.text(),
                test_ids: &self.test_ids,
                pixels_per_point: self.context.pixels_per_point(),
                now: self.context.now(),
                window: &self.asked,
                fullscreen: self.context.fullscreen(),
                actions: &self.actions,
            };
            let mut world = World {
                events: &mut self.events,
                simulation: platform.simulation(),
                context: &self.context,
            };
            automation.begin(&view, &mut world);
            if automation.take_close() {
                let accepted = self.app.close_requested();
                automation.closed(accepted);
                self.closing |= accepted;
            }
        }
        while let Ok((id, pick)) = self.picks.1.try_recv() {
            self.context.file_picked(id, pick);
        }
        if let Some(fullscreen) = self.reports.take_fullscreen() {
            self.context.fullscreen_changed(fullscreen);
        }
        let pixels_per_point = match self.reports.take_size() {
            Some(size) => {
                renderers.resize(size.width, size.height);
                size.pixels_per_point
            }
            None => pixels_per_point,
        };
        let Some(physical) = renderers.physical() else {
            self.events.clear();
            self.intake.incoming.clear();
            return None;
        };
        self.context.set_pixels_per_point(pixels_per_point);
        let scale = self.context.pixels_per_point();
        self.intake.drain(
            &mut Drain {
                pixels_per_point: scale,
                touch_emulation: self.context.touch_emulation(),
            },
            &mut self.events,
        );
        let screen = physical / scale;
        match renderers.screens() {
            Some(screens) => {
                self.context.set_screens(
                    screens
                        .iter()
                        .map(|screen| screen.scaled(scale.recip()))
                        .collect(),
                );
                self.reports_screens = true;
            }
            None if std::mem::take(&mut self.reports_screens) => {
                self.context.set_screens(Vec::new());
            }
            None => {}
        }
        let raw = RawInput {
            events: next_batch(&mut self.events),
        };
        let typed = raw
            .events
            .iter()
            .any(|event| matches!(event, Event::Ime(_) | Event::Text(_)));
        let app = &mut self.app;
        let mut output = self.context.run(raw, |context| {
            app.update(context, safe_rect(screen, safe_area, scale));
        });
        let title = &self.title;
        if self.publishing_accessibility {
            platform.publish_accessibility(&mut || output.accessibility_tree(title, screen));
        }
        if let Some(dump) = &mut self.accessibility {
            dump.update(output.accessibility_tree(title, screen));
        }

        if let Some(text) = &output.copied_text {
            platform.copy(text.clone());
        }
        for request in std::mem::take(&mut output.file_picks) {
            let answer = FilePickAnswer::new(request.id, self.picks.0.clone(), self.waker.clone());
            platform.pick_file(request, answer);
        }
        let delivered =
            output.paste == Some(PasteRequest::Shortcut) && platform.delivers_paste_shortcut();
        if output.paste.is_some() && !delivered && !output.window.paste_kinds.is_empty() {
            let answer = PasteAnswer::new(self.pastes.0.clone(), self.waker.clone());
            platform.paste(&output.window.paste_kinds, answer);
            while let Ok(pasted) = self.pastes.1.try_recv() {
                self.events.push(Event::Paste(pasted));
            }
        }
        let window_units = scale / self.context.native_pixels_per_point();
        let window = output.window.clone().scaled(window_units);
        if typed || window != self.shown {
            platform.show_window(&window, &self.shown);
            self.shown = window;
        }
        self.asked.clone_from(&output.window);
        if let Some(fullscreen) = output.fullscreen
            && fullscreen != self.context.fullscreen()
        {
            platform.set_fullscreen(fullscreen);
        }

        let pending = renderers.prepare(&output, scale, self.app.clear_color());
        let mut frame = Frame {
            pending,
            deferred: !self.events.is_empty() || self.reports.pending(),
            repaint: output.repaint,
            repaint_after: output.repaint_after,
        };
        let closing = output.close_requested || self.closing;
        self.unpresented |= pending;
        if let Some(automation) = &mut self.automation
            && let Some(dump) = &self.accessibility
        {
            self.test_ids.clone_from(output.test_ids());
            self.actions = std::mem::take(&mut output.actions);
            let view = View {
                lines: dump.lines(),
                tree: dump.text(),
                test_ids: &self.test_ids,
                pixels_per_point: scale,
                now: self.context.now(),
                window: &self.asked,
                fullscreen: self.context.fullscreen(),
                actions: &self.actions,
            };
            let settled = Settled {
                busy: self.app.busy(),
                again: match automation.clock_stopped() {
                    true => frame.deferred,
                    false => frame.again(),
                },
            };
            let mut world = World {
                events: &mut self.events,
                simulation: platform.simulation(),
                context: &self.context,
            };
            automation.end(&view, settled, &mut world, &output.unran_actions);
            if let Some(after) = automation.wants_frame(self.context.now()) {
                frame.repaint |= after.is_zero();
                frame.repaint_after = frame.repaint_after.min(after);
            }
            if automation.wants_capture() && !self.unpresented {
                automation.captured(renderers.capture());
            }
        }
        self.output = Some(output);
        if closing {
            self.exit();
            return None;
        }
        Some(frame)
    }

    pub fn present(&mut self) -> bool {
        if self.exited {
            return false;
        }
        self.contain(Self::run_present).unwrap_or(false)
    }

    fn run_present(&mut self) -> bool {
        let background = self.app.clear_color();
        let Some(renderers) = self.renderers.as_mut() else {
            return false;
        };
        let again = renderers.present(background);
        if !again {
            self.unpresented = false;
            if let Some(automation) = &mut self.automation
                && automation.wants_capture()
            {
                automation.captured(renderers.capture());
            }
        }
        again
    }
}

pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    match (
        payload.downcast_ref::<&str>(),
        payload.downcast_ref::<String>(),
    ) {
        (Some(message), _) => (*message).to_owned(),
        (None, Some(message)) => message.clone(),
        (None, None) => "a panic without a message".to_owned(),
    }
}
