use std::any::Any;
use std::cell::Ref;
use std::error::Error;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use accesskit::TreeUpdate;

use crate::app::accessibility_dump::AccessibilityDump;
use crate::app::automation::{Automation, Inbox, Resize, Settled, View, World};
use crate::app::{App, SafeArea, Setup, Waker, next_batch, safe_rect};
use crate::clipboard::{PasteAnswer, PasteKind, PasteRequest, Pasted};
use crate::context::{ActionGroup, Context, FrameOutput};
use crate::file_picker::{FilePick, FilePickAnswer, FilePickId, FilePickRequest};
use crate::file_saver::{FileSave, FileSaveAnswer, FileSaveId, FileSaveRequest};
use crate::geometry::Vec2;
use crate::input::{Event, Input};
use crate::renderer::{Loaded, Renderers};
use crate::window::{WindowId, WindowReport, WindowReports, WindowState};

pub mod environment;
mod host;
mod intake;

pub use host::{HostView, Hosted, HostedPlatform, WindowHost};
use intake::{Drain, Intake};
pub use intake::{Incoming, PINCH_SPEED, SCROLL_LINE, SCROLL_PAGE, Units, scrolled};

type Answered = (FilePickId, FilePick);
type Saved = (FileSaveId, FileSave);

pub struct RunOptions {
    pub app_id: Option<String>,
    pub size: Vec2,
    pub accessibility_tree: bool,
    pub automation: Option<Inbox>,
    pub saved_preferences: Option<std::path::PathBuf>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            app_id: None,
            size: Vec2::new(1280.0, 800.0),
            accessibility_tree: false,
            automation: None,
            saved_preferences: None,
        }
    }
}

impl RunOptions {
    pub fn saving_preferences(mut self, app: &str) -> Self {
        self.saved_preferences =
            config_directory().map(|directory| directory.join(app).join("preferences.json"));
        self
    }
}

fn config_directory() -> Option<std::path::PathBuf> {
    let variable = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
    };
    if cfg!(target_os = "windows") {
        return variable("APPDATA");
    }
    if cfg!(target_os = "macos") {
        return variable("HOME").map(|home| home.join("Library/Application Support"));
    }
    variable("XDG_CONFIG_HOME").or_else(|| variable("HOME").map(|home| home.join(".config")))
}

pub struct Launch {
    pub options: RunOptions,
    pub context: Context,
    pub app: Box<dyn App>,
    pub host: Option<Box<dyn WindowHost>>,
}

pub type Running = Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>;

pub trait Platform {
    fn name(&self) -> &'static str;

    fn run(self: Box<Self>, launch: Launch) -> Running;
}

pub trait PlatformServices {
    fn copy(&mut self, _text: String) {}

    fn copy_image(&mut self, _png: Vec<u8>) {}

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        let _ = kinds;
        answer.send(None);
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        let _ = request;
        answer.send(Err("Choosing a file is unavailable here".to_owned()));
    }

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        let _ = request;
        answer.send(FileSave::Failed(
            "Saving a file is unavailable here".to_owned(),
        ));
    }

    fn delivers_paste_shortcut(&self) -> bool {
        false
    }

    fn shows_keyboard(&self) -> bool {
        false
    }

    fn show_window(&mut self, _window: &WindowState, _shown: &WindowState) {}

    fn resize(&mut self, _resize: Resize) -> Result<(), String> {
        Err("this window cannot be resized from here; run the app headless".to_owned())
    }

    fn set_fullscreen(&mut self, _fullscreen: bool) {}

    fn publish_accessibility(&mut self, _tree: &mut dyn FnMut() -> TreeUpdate) {}
}

pub struct Frame {
    pub pending: bool,
    pub deferred: bool,
    pub repaint: bool,
    pub repaint_after: Duration,
    pub animating: bool,
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
    host: Option<Box<dyn WindowHost>>,
    context: Context,
    renderers: Option<Renderers>,
    waker: Option<Waker>,
    events: Vec<Event>,
    intake: Intake,
    picks: (Sender<Answered>, Receiver<Answered>),
    saves: (Sender<Saved>, Receiver<Saved>),
    pastes: (Sender<Pasted>, Receiver<Pasted>),
    typed_pastes: (Sender<Pasted>, Receiver<Pasted>),
    keyboard_chosen: bool,
    publishing_accessibility: bool,
    automation: Option<Automation>,
    actions: Vec<ActionGroup>,
    unpresented: bool,
    occluded: bool,
    closing: bool,
    asked: WindowState,
    shown: WindowState,
    background: crate::color::Color32,
    reports: WindowReports,
    native: f32,
    output: Option<FrameOutput>,
    exited: bool,
    failure: Option<String>,
}

fn type_pastes(
    pastes: &Receiver<Pasted>,
    held: crate::input::Modifiers,
    incoming: &mut Vec<Incoming>,
) {
    while let Ok(pasted) = pastes.try_recv() {
        if let Pasted::Text(text) = pasted {
            let mut typed = Vec::new();
            crate::app::automation::keys::typed(&text, held, &mut typed);
            incoming.extend(typed.into_iter().map(Incoming::from));
        }
    }
}

impl Runner {
    pub fn new(launch: Launch) -> Self {
        let Launch {
            options,
            context,
            app,
            host,
        } = launch;
        context.set_window_hosting(match host.is_some() {
            true => crate::window::WindowHosting::Any,
            false => crate::window::WindowHosting::None,
        });
        let automation = options.automation.map(Automation::new);
        context.keep_accessibility(options.accessibility_tree || automation.is_some());
        if automation.is_some() {
            context.set_automated(true);
        }
        Self {
            app,
            host,
            context,
            renderers: None,
            waker: None,
            events: Vec::new(),
            intake: Intake::default(),
            picks: mpsc::channel(),
            saves: mpsc::channel(),
            pastes: mpsc::channel(),
            typed_pastes: mpsc::channel(),
            keyboard_chosen: false,
            publishing_accessibility: false,
            automation,
            actions: Vec::new(),
            unpresented: false,
            occluded: false,
            closing: false,
            asked: WindowState::default(),
            shown: WindowState::default(),
            background: crate::color::Color32::BLACK,
            reports: WindowReports::default(),
            native: 1.0,
            output: None,
            exited: false,
            failure: None,
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn app(&self) -> &dyn App {
        &*self.app
    }

    pub fn app_mut(&mut self) -> &mut dyn App {
        &mut *self.app
    }

    pub fn start(&mut self, loaded: Vec<Loaded>, mut setup: Setup) -> Result<(), Box<dyn Error>> {
        let renderers = Renderers::new(&self.context, loaded)?;
        renderers.provide(&mut setup);
        setup.provide(self.context.clone());
        self.renderers = Some(renderers);
        self.waker = Some(setup.waker.clone());
        self.context.set_waker(setup.waker.clone());
        if let Some(automation) = &self.automation {
            automation.inbox().set_waker(setup.waker.clone());
        }
        self.app.setup(&setup);
        if let Some(host) = &mut self.host {
            host.setup(&setup);
        }
        Ok(())
    }

    pub fn started(&self) -> bool {
        self.renderers.is_some()
    }

    pub fn renderers(&mut self) -> Option<&mut Renderers> {
        self.renderers.as_mut()
    }

    pub fn push(&mut self, input: impl Into<Incoming>) {
        self.intake.incoming.push(input.into());
    }

    pub fn modifiers(&self) -> crate::input::Modifiers {
        self.intake.modifiers()
    }

    pub fn has_events(&self) -> bool {
        !self.events.is_empty() || !self.intake.incoming.is_empty() || self.reports.pending()
    }

    pub fn set_occluded(&mut self, occluded: bool) {
        self.occluded = occluded;
    }

    pub fn reports(&self) -> &WindowReports {
        &self.reports
    }

    pub fn window(&self) -> &WindowState {
        &self.shown
    }

    pub fn take_output(&mut self) -> Option<FrameOutput> {
        self.output.take()
    }

    pub fn keep_accessibility(&mut self) {
        self.context.keep_accessibility(true);
    }

    pub fn accessibility_requested(&mut self) {
        self.publishing_accessibility = true;
        self.context.set_accessibility_requested(true);
        self.context.reset_accessibility();
    }

    pub fn accessibility_deactivated(&mut self) {
        self.publishing_accessibility = false;
        self.context.set_accessibility_requested(false);
    }

    pub fn accessibility_action(&mut self, request: accesskit::ActionRequest) {
        self.context.accessibility_action(request);
    }

    pub fn accessibility(&self) -> Option<Ref<'_, AccessibilityDump>> {
        self.context.kept_accessibility()
    }

    pub fn output(&self) -> Option<&FrameOutput> {
        self.output.as_ref()
    }

    pub fn accessibility_text(&self) -> Option<String> {
        self.accessibility()
            .map(|dump| dump.rendered().text().to_owned())
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
        Ok(self.outcome()?)
    }

    pub fn outcome(&mut self) -> Result<(), String> {
        self.exit();
        match self.failure.take() {
            Some(failure) => Err(format!("the app stopped after a panic: {failure}")),
            None => Ok(()),
        }
    }

    pub fn frame(
        &mut self,
        platform: &mut dyn PlatformServices,
        safe_area: SafeArea,
    ) -> Option<Frame> {
        if self.exited {
            return None;
        }
        self.contain(|runner| runner.run_frame(platform, safe_area))
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
        platform: &mut dyn PlatformServices,
        safe_area: SafeArea,
    ) -> Option<Frame> {
        let started = std::time::Instant::now();
        let renderers = self.renderers.as_mut()?;
        match renderers.recover() {
            Ok(false) => {}
            Ok(true) => {
                if let Some(waker) = &self.waker {
                    let mut setup = Setup::new(waker.clone());
                    renderers.provide(&mut setup);
                    setup.provide(self.context.clone());
                    self.app.renderer_replaced(&setup);
                    if let Some(host) = &mut self.host {
                        host.setup(&setup);
                    }
                }
            }
            Err(error) => panic!("beui: the GPU was lost and could not be reopened: {error}"),
        }
        while let Ok(pasted) = self.pastes.1.try_recv() {
            self.intake.incoming.push(Input::Paste(pasted).into());
        }
        let held = self.intake.modifiers();
        type_pastes(&self.typed_pastes.1, held, &mut self.intake.incoming);
        if !std::mem::replace(&mut self.keyboard_chosen, true) && !platform.shows_keyboard() {
            self.context.enable_keyboard();
        }
        if let Err(error) = renderers.follow_choice(&self.context) {
            eprintln!("beui: could not switch renderers: {error}");
        }
        if let Some(automation) = &mut self.automation
            && let Some(dump) = self.context.kept_accessibility()
        {
            let view = View {
                accessibility: &dump,
                pixels_per_point: self.context.pixels_per_point(),
                now: self.context.now(),
                size: renderers.physical().map_or(Vec2::ZERO, |physical| {
                    physical / self.context.pixels_per_point()
                }),
                window: &self.asked,
                fullscreen: self.context.fullscreen(),
                actions: &self.actions,
                environment: &|| self.app.environment(),
            };
            let mut world = World {
                events: &mut self.intake.incoming,
                platform: &mut *platform,
                context: &self.context,
            };
            automation.begin(&view, &mut world);
            if automation.take_painting_request() {
                self.app.request_painting();
            }
            if automation.take_close() {
                let accepted = self.app.close_requested();
                automation.closed(accepted);
                self.closing |= accepted;
            }
        }
        while let Ok((id, pick)) = self.picks.1.try_recv() {
            self.context.file_picked(id, pick);
        }
        while let Ok((id, save)) = self.saves.1.try_recv() {
            self.context.file_saved(id, save);
        }
        if let Some(fullscreen) = self.reports.take_fullscreen() {
            self.context.fullscreen_changed(fullscreen);
        }
        if let Some((width, height)) = self.reports.take_size() {
            renderers.resize(width, height);
        }
        if let Some(scale) = self.reports.take_scale() {
            self.native = scale;
        }
        if let Some(attached) = self.reports.take_keyboard() {
            self.context.set_keyboard_attached(attached);
        }
        if let Some(preferences) = self.reports.take_system_preferences() {
            self.context
                .set_preference_layer(crate::preferences::PreferenceSource::System, preferences);
        }
        let pixels_per_point = self.native;
        let Some(physical) = renderers.physical() else {
            self.events.clear();
            self.intake.incoming.clear();
            return None;
        };
        self.context.set_pixels_per_point(pixels_per_point);
        let scale = self.context.pixels_per_point();
        let view = HostView {
            platform: physical,
            pixels_per_point: scale,
            now: self.context.frame_time(),
        };
        if let Some(host) = &mut self.host {
            host.input(&mut self.intake.incoming, &view);
        }
        self.intake.drain(
            &mut Drain {
                pixels_per_point: scale,
                native_pixels_per_point: self.native,
                touch_emulation: self.context.touch_emulation(),
                window_size: &|window| {
                    self.context
                        .window_size(window)
                        .or((window == WindowId::MAIN).then_some(physical / scale))
                },
            },
            &mut self.events,
        );
        for report in self.intake.reports.drain(..) {
            self.context.window_reported(report);
        }
        let (screen, safe_area) = match &self.host {
            Some(_) => (
                self.context
                    .window_size(WindowId::MAIN)
                    .unwrap_or(physical / scale),
                SafeArea::default(),
            ),
            None => {
                let screen = physical / scale;
                self.context.window_reported(WindowReport::Resized {
                    window: WindowId::MAIN,
                    size: screen,
                });
                (screen, safe_area)
            }
        };
        let events = next_batch(&mut self.events);
        let seen = !self.occluded
            || self
                .automation
                .as_ref()
                .is_some_and(Automation::wants_sight);
        self.context.set_seen(seen);
        self.context.set_accessibility_viewport(screen);
        let app = &mut self.app;
        let mut output = self.context.run_with(events, view.now, |context| {
            app.update(context, safe_rect(screen, safe_area, scale));
        });
        if self.publishing_accessibility {
            let title = output
                .main()
                .window
                .appearance
                .title
                .as_deref()
                .unwrap_or_default();
            platform.publish_accessibility(&mut || output.accessibility_tree(title, screen));
        }

        if let Some(text) = &output.copied_text {
            platform.copy(text.clone());
        }
        if let Some(png) = output.copied_image.take() {
            platform.copy_image(png);
        }
        for request in std::mem::take(&mut output.file_picks) {
            let answer = FilePickAnswer::new(request.id, self.picks.0.clone(), self.waker.clone());
            match &mut self.automation {
                Some(automation) if !cfg!(target_arch = "wasm32") => {
                    automation.pick_file(request, answer);
                }
                _ => platform.pick_file(request, answer),
            }
        }
        for request in std::mem::take(&mut output.file_saves) {
            let answer = FileSaveAnswer::new(request.id, self.saves.0.clone(), self.waker.clone());
            match &mut self.automation {
                Some(automation) if !cfg!(target_arch = "wasm32") => {
                    automation.save_file(request, answer);
                }
                _ => platform.save_file(request, answer),
            }
        }
        self.intake
            .incoming
            .extend(output.typed.drain(..).map(Incoming::from));
        if output.paste == Some(PasteRequest::Typed) {
            let answer = PasteAnswer::new(self.typed_pastes.0.clone(), self.waker.clone());
            platform.paste(&[crate::clipboard::PasteKind::Text], answer);
            let held = self.intake.modifiers();
            type_pastes(&self.typed_pastes.1, held, &mut self.intake.incoming);
        }
        let delivered =
            output.paste == Some(PasteRequest::Shortcut) && platform.delivers_paste_shortcut();
        if output
            .paste
            .is_some_and(|paste| paste != PasteRequest::Typed)
            && !delivered
            && !output.main().window.requests.pastes.is_empty()
        {
            let answer = PasteAnswer::new(self.pastes.0.clone(), self.waker.clone());
            platform.paste(&output.main().window.requests.pastes, answer);
            while let Ok(pasted) = self.pastes.1.try_recv() {
                self.intake.incoming.push(Input::Paste(pasted).into());
            }
        }
        let hosted = self.host.as_mut().map(|host| host.frame(&output, &view));
        let shown = hosted
            .as_ref()
            .map_or_else(|| output.main(), |hosted| &hosted.window);
        let window_units = scale / self.context.native_pixels_per_point();
        let window = shown.window.clone().scaled(window_units);
        if window != self.shown {
            platform.show_window(&window, &self.shown);
            self.shown = window;
        }
        self.asked.clone_from(&output.main().window);
        self.background = shown.window.appearance.clear_color;
        if let Some(fullscreen) = output.fullscreen
            && fullscreen != self.context.fullscreen()
        {
            platform.set_fullscreen(fullscreen);
        }

        let background = self.background;
        let pending = match seen {
            true => renderers.prepare(shown, scale, background),
            false => {
                renderers.note(shown, scale, background);
                false
            }
        };
        let repaint_after = hosted.as_ref().map_or(output.repaint_after, |hosted| {
            output.repaint_after.min(hosted.repaint_after)
        });
        let mut frame = Frame {
            pending,
            deferred: !self.events.is_empty()
                || !self.intake.incoming.is_empty()
                || self.reports.pending(),
            repaint: output.repaint,
            repaint_after,
            animating: output.animating,
        };
        let closing = output.close_requested || self.closing;
        self.unpresented |= pending;
        if let Some(automation) = &mut self.automation
            && let Some(dump) = self.context.kept_accessibility()
        {
            let took = started.elapsed();
            let others = self.app.other_frames();
            self.actions = std::mem::take(&mut output.actions);
            let view = View {
                accessibility: &dump,
                pixels_per_point: scale,
                now: self.context.now(),
                size: screen,
                window: &self.asked,
                fullscreen: self.context.fullscreen(),
                actions: &self.actions,
                environment: &|| self.app.environment(),
            };
            let settled = Settled {
                busy: self.app.busy(),
                drawing: self.app.drawing(),
                again: match automation.clock_stopped() {
                    true => frame.deferred,
                    false => frame.again(),
                },
            };
            automation.framed(took, others);
            automation.end(
                &view,
                settled,
                &output.unran_actions,
                output.hits.as_deref(),
                output.comps.as_deref(),
            );
            if let Some(after) = automation.wants_frame(self.context.now()) {
                frame.repaint |= after.is_zero();
                frame.repaint_after = frame.repaint_after.min(after);
            }
            if automation.wants_capture() && !self.unpresented {
                automation.captured(renderers.capture());
            }
            if automation.wants_painting() {
                automation.painted(crate::painting::painting(
                    output.main(),
                    screen,
                    scale,
                    output.main().window.appearance.clear_color,
                    crate::painting::Punches::Erase,
                ));
            }
            if automation.wants_overflows() {
                automation.overflowed(crate::painting::overflows(&output, scale));
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
        if self.occluded && !self.unpresented {
            return false;
        }
        let background = self.background;
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
