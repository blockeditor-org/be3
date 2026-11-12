use std::any::Any;
use std::cell::Ref;
use std::error::Error;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use accesskit::TreeUpdate;

use crate::renderer::{Capture, Loaded, Rendering};
use beui_core::clipboard::{Clip, ClipboardRead, PasteAnswer, PasteRequest};
use beui_core::context::{Context, FrameOutput, WindowOutput};
use beui_core::file_picker::{FilePickAnswer, FilePickId, FilePickRequest};
use beui_core::file_saver::{FileSaveAnswer, FileSaveId, FileSaveRequest};
use beui_core::program::accessibility_dump::AccessibilityDump;
use beui_core::program::{Program, SafeArea, Setup, Waker, next_batch, safe_rect};
use beui_core::window::{Resize, WindowReports, WindowState};
use beui_types::ClipItem;
use beui_types::Event;
use beui_types::FilePick;
use beui_types::FileSave;
use beui_types::Input;
use beui_types::Vec2;
use beui_types::clipboard::formats;
use beui_types::{WindowId, WindowReport};

mod controller;
pub mod environment;
mod intake;
mod layer;
pub mod renderer;

pub use controller::{Controller, Hand, HandPose};
use intake::{Drain, Intake};
pub use intake::{Incoming, PINCH_SPEED, SCROLL_LINE, SCROLL_PAGE, Units, scrolled};
pub use layer::{Above, Below, Layer, LayerFrame, LayerView, Shown};

type Answered = (FilePickId, FilePick);
type Saved = (FileSaveId, FileSave);

pub struct RunOptions {
    pub program_id: Option<String>,
    pub size: Vec2,
    pub accessibility_tree: bool,
    pub saved_preferences: Option<std::path::PathBuf>,
    pub device_extensions: &'static [&'static std::ffi::CStr],
    pub platforms: Vec<Box<dyn Platform>>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            program_id: None,
            size: Vec2::new(1280.0, 800.0),
            accessibility_tree: false,
            saved_preferences: None,
            device_extensions: &[],
            platforms: Vec::new(),
        }
    }
}

impl RunOptions {
    pub fn saving_preferences(mut self, program: &str) -> Self {
        self.saved_preferences =
            config_directory().map(|directory| directory.join(program).join("preferences.json"));
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
    pub program: Box<dyn Program>,
    pub layers: Vec<Box<dyn Layer>>,
}

pub type Running = Pin<Box<dyn Future<Output = Result<(), Box<dyn Error>>>>>;

/// Owns the event loop and only translates native input into `Incoming`s for
/// `Runner::push`; everything a frame does is `Runner::frame`, shared by every
/// platform. A new platform implements this and `PlatformServices`, and hands its
/// window or element to the renderer loader beui-host gives it.
pub trait Platform {
    fn name(&self) -> &'static str;

    fn run(self: Box<Self>, launch: Launch) -> Running;
}

/// What the program asks of the window. Every method has a default, so a
/// platform implements only what it has; layers receive the services below them
/// and forward to them by default.
pub trait PlatformServices {
    /// Puts `clip` on the clipboard. A platform keeps it to answer pastes of
    /// what it cannot put on the system clipboard (lazy representations, be3
    /// formats), until another program copies.
    fn copy(&mut self, _clip: Clip) {}

    /// Answers with the first of `formats` the clipboard holds.
    fn paste(&mut self, formats: &[String], answer: PasteAnswer) {
        let _ = formats;
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

    /// True where Ctrl+V already arrives as a paste (the browser's paste event, an
    /// app's host), so the runner does not also ask `paste` for it.
    fn delivers_paste_shortcut(&self) -> bool {
        false
    }

    fn shows_keyboard(&self) -> bool {
        false
    }

    /// Called with the new and previously shown state only when they differ, so a
    /// platform compares only the fields it acts on. A new `WindowState` field means
    /// changes here and in every platform that uses it.
    fn show_window(&mut self, _window: &WindowState, _shown: &WindowState) {}

    /// Only a headless window can be resized by the program or a driver.
    fn resize(&mut self, _resize: Resize) -> Result<(), String> {
        Err("this window cannot be resized from here; run the program headless".to_owned())
    }

    fn set_fullscreen(&mut self, _fullscreen: bool) {}

    fn raise_window(&mut self) {}

    fn publish_accessibility(&mut self, _tree: &mut dyn FnMut() -> TreeUpdate) {}
}

pub struct Frame {
    pub pending: bool,
    pub deferred: bool,
    pub repaint: bool,
    pub repaint_after: Duration,
    pub animating: bool,
    pub unpaced: bool,
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

impl Program for Exited {
    fn update(&mut self, _context: &Context, _rect: beui_types::Rect) {}
}

pub struct Runner {
    program: Box<dyn Program>,
    layers: Vec<Box<dyn Layer>>,
    shown: Option<Shown>,
    context: Context,
    rendering: Option<Rendering>,
    waker: Option<Waker>,
    events: Vec<Event>,
    /// Input that ends something, kept from frames that could not run for want
    /// of a surface, for the next frame that can. Not counted by `has_events`, so
    /// a platform does not spin while it waits for the surface.
    held: Vec<Incoming>,
    held_events: Vec<Event>,
    intake: Intake,
    picks: (Sender<Answered>, Receiver<Answered>),
    saves: (Sender<Saved>, Receiver<Saved>),
    pastes: (Sender<ClipItem>, Receiver<ClipItem>),
    typed_pastes: (Sender<ClipItem>, Receiver<ClipItem>),
    clipboard_reads: Vec<(Receiver<ClipItem>, ClipboardRead)>,
    keyboard_chosen: bool,
    publishing_accessibility: bool,
    unpresented: bool,
    occluded: bool,
    closing: bool,
    window: WindowState,
    background: beui_types::Color32,
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
    pastes: &Receiver<ClipItem>,
    held: beui_types::Modifiers,
    incoming: &mut Vec<Incoming>,
) {
    while let Ok(pasted) = pastes.try_recv() {
        if let Some(text) = pasted.as_text() {
            let mut typed = Vec::new();
            beui_types::input::typing::typed(&text, held, &mut typed);
            incoming.extend(typed.into_iter().map(Incoming::from));
        }
    }
}

impl Runner {
    pub fn new(launch: Launch) -> Self {
        let Launch {
            options,
            context,
            program,
            mut layers,
        } = launch;
        context.set_window_hosting(beui_core::window::WindowHosting::None);
        context.keep_accessibility(options.accessibility_tree);
        for layer in &mut layers {
            layer.attach(&context);
        }
        Self {
            program,
            layers,
            shown: None,
            context,
            rendering: None,
            waker: None,
            events: Vec::new(),
            held: Vec::new(),
            held_events: Vec::new(),
            intake: Intake::default(),
            picks: mpsc::channel(),
            saves: mpsc::channel(),
            pastes: mpsc::channel(),
            typed_pastes: mpsc::channel(),
            clipboard_reads: Vec::new(),
            keyboard_chosen: false,
            publishing_accessibility: false,
            unpresented: false,
            occluded: false,
            closing: false,
            window: WindowState::default(),
            background: beui_types::color::Color32::BLACK,
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

    pub fn program(&self) -> &dyn Program {
        &*self.program
    }

    pub fn program_mut(&mut self) -> &mut dyn Program {
        &mut *self.program
    }

    pub fn start(&mut self, loaded: Loaded, mut setup: Setup) -> Result<(), Box<dyn Error>> {
        let rendering = Rendering::new(&self.context, loaded);
        rendering.provide(&mut setup);
        setup.provide(self.context.clone());
        self.rendering = Some(rendering);
        self.waker = Some(setup.waker.clone());
        self.context.set_waker(setup.waker.clone());
        self.program.setup(&setup);
        for layer in &mut self.layers {
            layer.setup(&setup);
        }
        Ok(())
    }

    pub fn started(&self) -> bool {
        self.rendering.is_some()
    }

    pub fn rendering(&mut self) -> Option<&mut Rendering> {
        self.rendering.as_mut()
    }

    pub fn push(&mut self, input: impl Into<Incoming>) {
        self.intake.incoming.push(input.into());
    }

    pub fn modifiers(&self) -> beui_types::Modifiers {
        self.intake.modifiers()
    }

    pub fn has_events(&self) -> bool {
        !self.events.is_empty() || !self.intake.incoming.is_empty() || self.reports.pending()
    }

    /// An occluded window still runs every frame (layout, paint, accessibility, a
    /// driver), but each frame's output goes to `Renderer::note` instead of
    /// `prepare`, and nothing is presented. The headless platform is always occluded.
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

    /// Trees are published only between this and `accessibility_deactivated`, so a
    /// host asks what it embeds to describe itself only while something listens.
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

    /// For a window closed from outside: asks the program's `close_requested`, and
    /// exits only if it agrees.
    pub fn os_close(&mut self) -> bool {
        let accepted = self
            .contain(|runner| runner.program.close_requested())
            .unwrap_or(true);
        if accepted {
            self.exit();
        }
        accepted
    }

    pub fn lifecycle(&mut self, change: beui_core::program::Lifecycle) {
        if !self.exited {
            self.contain(|runner| runner.program.lifecycle(change));
        }
    }

    /// Runs `Program::exiting` and drops the program while the GPU and displays still
    /// exist. A panic in the program (here or in `update`) is caught and kept as
    /// `failure` rather than unwinding through the platform's event loop.
    pub fn exit(&mut self) {
        if !self.exited {
            self.exited = true;
            let program = &mut self.program;
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| program.exiting())) {
                self.failure
                    .get_or_insert_with(|| format!("while exiting: {}", panic_message(&*payload)));
            }
            self.program = Box::new(Exited);
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
            Some(failure) => Err(format!("the program stopped after a panic: {failure}")),
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
        let rendering = self.rendering.as_mut()?;
        match rendering.recover() {
            Ok(false) => {}
            Ok(true) => {
                if let Some(waker) = &self.waker {
                    let mut setup = Setup::new(waker.clone());
                    rendering.provide(&mut setup);
                    setup.provide(self.context.clone());
                    self.program.renderer_replaced(&setup);
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
        for (pasted, read) in std::mem::take(&mut self.clipboard_reads) {
            match pasted.try_recv() {
                Ok(item) => read.answer(Some(item)),
                Err(mpsc::TryRecvError::Disconnected) => read.answer(None),
                Err(mpsc::TryRecvError::Empty) => self.clipboard_reads.push((pasted, read)),
            }
        }
        if !std::mem::replace(&mut self.keyboard_chosen, true)
            && !services(&mut self.layers, platform).shows_keyboard()
        {
            self.context.enable_keyboard();
        }
        if let Some(fullscreen) = self.reports.take_fullscreen() {
            self.context.fullscreen_changed(fullscreen);
        }
        if let Some((width, height)) = self.reports.take_size() {
            rendering.resize(width, height);
        }
        if let Some(scale) = self.reports.take_scale() {
            self.native = scale;
        }
        if let Some(interval) = self.reports.take_refresh() {
            self.context.set_refresh_interval(Some(interval));
        }
        if let Some(attached) = self.reports.take_keyboard() {
            self.platform_keyboard = Some(attached);
        }
        if let Some(attached) = self.reports.take_touchscreen() {
            self.context.set_touchscreen_attached(attached);
        }
        if let Some(preferences) = self.reports.take_system_preferences() {
            self.context.set_preference_layer(
                beui_types::preferences::PreferenceSource::System,
                preferences,
            );
        }
        let pixels_per_point = self.native;
        let visible = rendering.physical();
        let paced = match (visible, self.last_physical) {
            (Some(_), _) | (None, Some(_)) => self
                .layers
                .iter_mut()
                .fold(None, |paced, layer| paced.or(layer.pace())),
            (None, None) => None,
        };
        let Some(physical) = visible.or(self.last_physical.filter(|_| paced.is_some())) else {
            self.events.retain(intake::event_ends_something);
            self.held_events.append(&mut self.events);
            self.intake.incoming.retain(intake::ends_something);
            self.held.append(&mut self.intake.incoming);
            return None;
        };
        self.last_physical = Some(physical);
        if !self.held.is_empty() {
            self.intake
                .incoming
                .splice(0..0, std::mem::take(&mut self.held));
        }
        if !self.held_events.is_empty() {
            self.events
                .splice(0..0, std::mem::take(&mut self.held_events));
        }
        self.context.set_pixels_per_point(pixels_per_point);
        let scale = self.context.pixels_per_point();
        rendering.set_vsync(paced.is_none());
        self.context.set_next_present(paced.or_else(|| {
            self.context
                .refresh_interval()
                .map(|interval| started + interval)
        }));
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
            program: &mut *self.program,
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
                layer_target: &|layer| self.context.layer_target(layer),
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
        let program = &mut self.program;
        let mut output = self.context.run_with(events, view.now, |context| {
            program.update(context, safe_rect(screen, safe_area, scale));
        });
        self.intake.every_sample = output
            .windows
            .iter()
            .filter(|window| window.window.requests.every_sample)
            .map(|window| window.id)
            .collect();
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
        if let Some(clip) = &output.copied {
            below.copy(clip.clone());
        }
        for read in std::mem::take(&mut output.clipboard_reads) {
            let (sender, pasted) = mpsc::channel();
            below.paste(&read.formats, PasteAnswer::new(sender, self.waker.clone()));
            self.clipboard_reads.push((pasted, read));
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
            below.paste(&[formats::TEXT.to_owned()], answer);
            let held = self.intake.modifiers();
            type_pastes(&self.typed_pastes.1, held, &mut self.intake.incoming);
        }
        let delivered = matches!(output.paste, Some(PasteRequest::Shortcut { .. }))
            && below.delivers_paste_shortcut();
        let wanted = match output.paste {
            Some(PasteRequest::Shortcut { plain: true }) => output
                .main()
                .window
                .requests
                .pastes
                .iter()
                .filter(|format| *format == formats::TEXT)
                .cloned()
                .collect(),
            Some(PasteRequest::Asked | PasteRequest::Shortcut { .. }) => {
                output.main().window.requests.pastes.clone()
            }
            Some(PasteRequest::Typed) | None => Vec::new(),
        };
        if !delivered && !wanted.is_empty() {
            let answer = PasteAnswer::new(self.pastes.0.clone(), self.waker.clone());
            below.paste(&wanted, answer);
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
                program: &mut *self.program,
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
        if output.raise_requested {
            services(&mut self.layers, platform).raise_window();
        }

        let background = self.background;
        let pending = match seen {
            true => rendering.prepare(shown, scale, background),
            false => {
                rendering.note(shown, scale, background);
                false
            }
        };
        self.context.note_prepared(std::time::Instant::now());
        let mut frame = Frame {
            pending,
            deferred: !self.events.is_empty()
                || !self.intake.incoming.is_empty()
                || self.reports.pending(),
            repaint: output.repaint,
            repaint_after: output.repaint_after,
            animating: output.animating,
            unpaced: false,
        };
        Visit {
            layers: &mut self.layers,
            platform: &mut *platform,
            context: &self.context,
            program: &mut *self.program,
            view,
            started,
            closing: &mut self.closing,
        }
        .downward(|layer, layer_frame| layer.framed(&mut frame, &output, layer_frame));
        self.unpresented |= pending;
        if !self.unpresented && self.layers.iter().any(|layer| layer.wants_capture()) {
            capture_for(&mut self.layers, rendering.capture());
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
        let Some(rendering) = self.rendering.as_mut() else {
            return false;
        };
        let again = rendering.present(background);
        if !again {
            self.unpresented = false;
            if self.layers.iter().any(|layer| layer.wants_capture()) {
                capture_for(&mut self.layers, rendering.capture());
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
    program: &'a mut dyn Program,
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
                program: &mut *self.program,
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
