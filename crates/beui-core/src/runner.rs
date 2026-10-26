use std::any::Any;
use std::cell::Ref;
use std::error::Error;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use accesskit::TreeUpdate;

use crate::app::accessibility_dump::AccessibilityDump;
use crate::app::{App, SafeArea, Setup, Waker, next_batch, safe_rect};
use crate::clipboard::{PasteAnswer, PasteKind, PasteRequest, Pasted};
use crate::context::{Context, FrameOutput, WindowOutput};
use crate::file_picker::{FilePick, FilePickAnswer, FilePickId, FilePickRequest};
use crate::file_saver::{FileSave, FileSaveAnswer, FileSaveId, FileSaveRequest};
use crate::geometry::Vec2;
use crate::input::{Event, Input};
use crate::renderer::{Capture, Loaded, Renderers};
use crate::window::{Resize, WindowId, WindowReport, WindowReports, WindowState};

mod controller;
pub mod environment;
mod intake;
mod layer;

pub use controller::{Controller, Hand, HandPose};
use intake::{Drain, Intake};
pub use intake::{Incoming, PINCH_SPEED, SCROLL_LINE, SCROLL_PAGE, Units, scrolled};
pub use layer::{Above, Below, Layer, LayerFrame, LayerView, Shown};

type Answered = (FilePickId, FilePick);
type Saved = (FileSaveId, FileSave);

pub struct RunOptions {
    pub app_id: Option<String>,
    pub size: Vec2,
    pub accessibility_tree: bool,
    pub saved_preferences: Option<std::path::PathBuf>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            app_id: None,
            size: Vec2::new(1280.0, 800.0),
            accessibility_tree: false,
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
    pub layers: Vec<Box<dyn Layer>>,
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
    layers: Vec<Box<dyn Layer>>,
    shown: Option<Shown>,
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
    unpresented: bool,
    occluded: bool,
    closing: bool,
    window: WindowState,
    background: crate::color::Color32,
    reports: WindowReports,
    native: f32,
    paced: Option<Instant>,
    last_physical: Option<Vec2>,
    platform_keyboard: Option<bool>,
    keyboard_attached: Option<bool>,
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
            crate::input::typing::typed(&text, held, &mut typed);
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
            mut layers,
        } = launch;
        context.set_window_hosting(crate::window::WindowHosting::None);
        context.keep_accessibility(options.accessibility_tree);
        for layer in &mut layers {
            layer.attach(&context);
        }
        Self {
            app,
            layers,
            shown: None,
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
            unpresented: false,
            occluded: false,
            closing: false,
            window: WindowState::default(),
            background: crate::color::Color32::BLACK,
            reports: WindowReports::default(),
            native: 1.0,
            paced: None,
            last_physical: None,
            platform_keyboard: None,
            keyboard_attached: None,
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
        self.app.setup(&setup);
        for layer in &mut self.layers {
            layer.setup(&setup);
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
        &self.window
    }

    pub fn shown_window(&self, window: WindowId) -> Option<&WindowOutput> {
        match &self.shown {
            Some(shown) => shown.window(window),
            None => self.output.as_ref()?.window(window),
        }
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

    pub fn lifecycle(&mut self, change: crate::app::Lifecycle) {
        if !self.exited {
            self.contain(|runner| runner.app.lifecycle(change));
        }
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
                    for layer in &mut self.layers {
                        layer.setup(&setup);
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
        if !std::mem::replace(&mut self.keyboard_chosen, true)
            && !services(&mut self.layers, platform).shows_keyboard()
        {
            self.context.enable_keyboard();
        }
        if let Err(error) = renderers.follow_choice(&self.context) {
            eprintln!("beui: could not switch renderers: {error}");
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
            self.platform_keyboard = Some(attached);
        }
        if let Some(attached) = self.reports.take_touchscreen() {
            self.context.set_touchscreen_attached(attached);
        }
        if let Some(preferences) = self.reports.take_system_preferences() {
            self.context
                .set_preference_layer(crate::preferences::PreferenceSource::System, preferences);
        }
        let pixels_per_point = self.native;
        let visible = renderers.physical();
        let paced = match (visible, self.last_physical) {
            (Some(_), _) | (None, Some(_)) => self
                .layers
                .iter_mut()
                .fold(None, |paced, layer| paced.or(layer.pace())),
            (None, None) => None,
        };
        let Some(physical) = visible.or(self.last_physical.filter(|_| paced.is_some())) else {
            self.events.clear();
            self.intake.incoming.clear();
            return None;
        };
        self.last_physical = Some(physical);
        self.context.set_pixels_per_point(pixels_per_point);
        let scale = self.context.pixels_per_point();
        renderers.set_vsync(paced.is_none());
        let mut now = match paced {
            Some(at) if !self.context.clock_stopped() => {
                let at = self.paced.map_or(at, |last| at.max(last));
                self.paced = Some(at);
                at
            }
            _ => self.context.frame_time(),
        };
        for layer in &mut self.layers {
            now = layer.now(now);
        }
        let view = LayerView {
            platform: physical,
            pixels_per_point: scale,
            now,
        };
        let mut incoming = std::mem::take(&mut self.intake.incoming);
        Visit {
            layers: &mut self.layers,
            platform: &mut *platform,
            context: &self.context,
            app: &mut *self.app,
            view,
            started,
            closing: &mut self.closing,
        }
        .upward(|layer, frame| layer.input(&mut incoming, frame));
        self.intake.incoming = incoming;
        while let Ok((id, pick)) = self.picks.1.try_recv() {
            self.context.file_picked(id, pick);
        }
        while let Ok((id, save)) = self.saves.1.try_recv() {
            self.context.file_saved(id, save);
        }
        let attached = self
            .layers
            .iter()
            .rev()
            .find_map(|layer| layer.keyboard_attached())
            .or(self.platform_keyboard);
        if attached != self.keyboard_attached {
            self.keyboard_attached = attached;
            self.context.set_keyboard_attached(attached.unwrap_or(true));
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
        let (screen, safe_area) = match self.context.opens_windows() {
            true => (
                self.context
                    .window_size(WindowId::MAIN)
                    .unwrap_or(physical / scale),
                SafeArea::default(),
            ),
            false => {
                let screen = physical / scale;
                self.context.window_reported(WindowReport::Resized {
                    window: WindowId::MAIN,
                    size: screen,
                });
                (screen, safe_area)
            }
        };
        let events = next_batch(&mut self.events);
        let seen = (!self.occluded && visible.is_some())
            || self.layers.iter().any(|layer| layer.wants_sight());
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
            services(&mut self.layers, platform)
                .publish_accessibility(&mut || output.accessibility_tree(title, screen));
        }

        let mut below = services(&mut self.layers, platform);
        if let Some(text) = &output.copied_text {
            below.copy(text.clone());
        }
        if let Some(png) = output.copied_image.take() {
            below.copy_image(png);
        }
        for request in std::mem::take(&mut output.file_picks) {
            let answer = FilePickAnswer::new(request.id, self.picks.0.clone(), self.waker.clone());
            below.pick_file(request, answer);
        }
        for request in std::mem::take(&mut output.file_saves) {
            let answer = FileSaveAnswer::new(request.id, self.saves.0.clone(), self.waker.clone());
            below.save_file(request, answer);
        }
        self.intake
            .incoming
            .extend(output.typed.drain(..).map(Incoming::from));
        if output.paste == Some(PasteRequest::Typed) {
            let answer = PasteAnswer::new(self.typed_pastes.0.clone(), self.waker.clone());
            below.paste(&[crate::clipboard::PasteKind::Text], answer);
            let held = self.intake.modifiers();
            type_pastes(&self.typed_pastes.1, held, &mut self.intake.incoming);
        }
        let delivered =
            output.paste == Some(PasteRequest::Shortcut) && below.delivers_paste_shortcut();
        if output
            .paste
            .is_some_and(|paste| paste != PasteRequest::Typed)
            && !delivered
            && !output.main().window.requests.pastes.is_empty()
        {
            let answer = PasteAnswer::new(self.pastes.0.clone(), self.waker.clone());
            below.paste(&output.main().window.requests.pastes, answer);
            while let Ok(pasted) = self.pastes.1.try_recv() {
                self.intake.incoming.push(Input::Paste(pasted).into());
            }
        }
        let layered = (!self.layers.is_empty()).then(|| {
            let mut layered = Shown::of(&output);
            Visit {
                layers: &mut self.layers,
                platform: &mut *platform,
                context: &self.context,
                app: &mut *self.app,
                view,
                started,
                closing: &mut self.closing,
            }
            .downward(|layer, frame| layer.output(&mut layered, &output, frame));
            layered
        });
        let shown = layered
            .as_ref()
            .and_then(Shown::main)
            .unwrap_or_else(|| output.main());
        let window_units = scale / self.context.native_pixels_per_point();
        let window = shown.window.clone().scaled(window_units);
        if window != self.window {
            services(&mut self.layers, platform).show_window(&window, &self.window);
            self.window = window;
        }
        self.background = shown.window.appearance.clear_color;
        if let Some(fullscreen) = output.fullscreen
            && fullscreen != self.context.fullscreen()
        {
            services(&mut self.layers, platform).set_fullscreen(fullscreen);
        }

        let background = self.background;
        let pending = match seen {
            true => renderers.prepare(shown, scale, background),
            false => {
                renderers.note(shown, scale, background);
                false
            }
        };
        let mut frame = Frame {
            pending,
            deferred: !self.events.is_empty()
                || !self.intake.incoming.is_empty()
                || self.reports.pending(),
            repaint: output.repaint,
            repaint_after: output.repaint_after,
            animating: output.animating,
        };
        Visit {
            layers: &mut self.layers,
            platform: &mut *platform,
            context: &self.context,
            app: &mut *self.app,
            view,
            started,
            closing: &mut self.closing,
        }
        .downward(|layer, layer_frame| layer.framed(&mut frame, &output, layer_frame));
        self.unpresented |= pending;
        if !self.unpresented && self.layers.iter().any(|layer| layer.wants_capture()) {
            capture_for(&mut self.layers, renderers.capture());
        }
        let closing = output.close_requested || self.closing;
        self.shown = layered;
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
            if self.layers.iter().any(|layer| layer.wants_capture()) {
                capture_for(&mut self.layers, renderers.capture());
            }
        }
        again
    }
}

fn services<'a>(
    layers: &'a mut [Box<dyn Layer>],
    platform: &'a mut dyn PlatformServices,
) -> Below<'a> {
    Below { layers, platform }
}

fn capture_for(layers: &mut [Box<dyn Layer>], capture: Option<Capture>) {
    let mut wanting = layers
        .iter_mut()
        .filter(|layer| layer.wants_capture())
        .peekable();
    while let Some(layer) = wanting.next() {
        match wanting.peek() {
            Some(_) => layer.captured(capture.clone()),
            None => {
                layer.captured(capture);
                return;
            }
        }
    }
}

struct Visit<'a> {
    layers: &'a mut [Box<dyn Layer>],
    platform: &'a mut dyn PlatformServices,
    context: &'a Context,
    app: &'a mut dyn App,
    view: LayerView,
    started: Instant,
    closing: &'a mut bool,
}

impl Visit<'_> {
    fn upward(self, act: impl FnMut(&mut dyn Layer, &mut LayerFrame<'_>)) {
        let order = 0..self.layers.len();
        self.each(order, act);
    }

    fn downward(self, act: impl FnMut(&mut dyn Layer, &mut LayerFrame<'_>)) {
        let order = (0..self.layers.len()).rev();
        self.each(order, act);
    }

    fn each(
        self,
        order: impl Iterator<Item = usize>,
        mut act: impl FnMut(&mut dyn Layer, &mut LayerFrame<'_>),
    ) {
        for index in order {
            let (below, rest) = self.layers.split_at_mut(index);
            let Some((layer, above)) = rest.split_first_mut() else {
                continue;
            };
            let mut frame = LayerFrame {
                view: self.view,
                context: self.context,
                app: &mut *self.app,
                below: Below {
                    layers: below,
                    platform: &mut *self.platform,
                },
                above: Above { layers: above },
                started: self.started,
                closing: &mut *self.closing,
            };
            act(&mut **layer, &mut frame);
        }
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
