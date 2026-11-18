//! be-host with a root guest in place of its own UI (`--root=APP`;
//! plans/host_as_platform_adapter.md, step 4e5c): the staged app `APP`'s entry point runs as the
//! root (be-wasm-root-host's `Root`) on the window's device. Each host frame hands it the
//! window's input since the last, asks it for a frame, and draws the frame's layer list: each
//! screen it names by handle (`Surface::Screen`) is the texture `Root::surface` resolves, drawn
//! as `beui::native::compose::quad` places it, and so is each surface of the root's own
//! (`Surface::Root`), which the root draws on the window's device as an app draws
//! (`Root::own_surface`); where it draws nothing the window shows its plain background. The host
//! draws on demand: when the root asks for its next frame (`frame-done`'s `next`) or has
//! something to step for (`Root::on_wake`).
//!
//! This is the path step 4g turns on and makes the default, so it runs on be-host's real
//! startup path and window, through beui's runner; only what the window shows comes from the
//! root. The root takes the window's input as the platform delivered it, before beui makes
//! events of it (`Program::input`; step 4f1): beui's document hears none of it, so keys the root
//! does not route reach nothing in be-host. What the root asks of the window, its input method's
//! area, cursor shape, pointer lock and every sample of the pointer's moves, are the window's
//! requests (`frame-done`); each input it is handed carries the frame clock's time it was
//! sampled at, when the platform said.
//!
//! It runs none of the host's core: no data directory or the lock that keeps a second be-host
//! off it, no block stack, workspace or accounts, no screen lock, and none of the desktop's OS
//! services. Step 4g brings back under this program what the host keeps (the plan's What the
//! host keeps) before it becomes the default; until then `--root` is refused as a login session
//! (`BEUI_PLATFORM=drm`), which would have no lock screen.
//!
//! The root is the app: one that fails to start or stops closes the window, and be-host exits
//! saying why, as it would on a panic.

use std::{
    cell::RefCell,
    error::Error,
    rc::Rc,
    time::{Duration, Instant},
};

use be_lib_layers::{Layer, LayerList, Surface};
use be_lib_ui_types::input::Timed;
use be_sdk_app_host::pacing::FrameClock;
use be_wasm_root_api::FrameRequest;
use be_wasm_root_host::Root;
use beui::native::texture_quad::{Pipelines, Quad, Quads};
use beui::native::{Draw, DrawAt, Drawing as GpuDrawing, compose, draw_gpu};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, Input, WindowId};

/// The id what the root shows has in the accessibility tree, `#root` to drive.
pub(crate) const SHOWN: &str = "root";

/// Runs `app`'s entry point as the root in a window opened with `options`, until the window
/// closes or the root stops, which is an error.
pub(crate) fn run(
    app: String,
    options: beui_host::native::RunOptions,
) -> Result<(), Box<dyn Error>> {
    let stopped: Rc<RefCell<Option<String>>> = Rc::default();
    beui_host_window::run_with(options, RootShell::new(app, Rc::clone(&stopped)))?;
    match stopped.take() {
        Some(why) => Err(why.into()),
        None => Ok(()),
    }
}

struct RootShell {
    app: String,
    document: beui::native::Document,
    /// What the window shows of the root's last frame.
    shown: Signal<Option<GpuDrawing>>,
    /// The windows' input since the root was last handed it, each in its window's points, with
    /// when the platform sampled it, if it said.
    input: Vec<(WindowId, Input, Option<Instant>)>,
    root: Option<Root>,
    /// Where the frame clock's time the root is asked for starts.
    time_origin: Option<Instant>,
    /// Where the real time the frame deadlines are kept in starts.
    origin: Option<Instant>,
    /// When the root wants its next frame, in its time, as its last frame said.
    next: Option<Duration>,
    /// Whether the root owes work now, as its last frame said: be-host is busy while it does.
    owed: bool,
    /// The deadline each frame waits for the root's children until, as be-host's own UI
    /// keeps it (`FrameClock`): with drive's clock stopped, a long one, so which frame shows
    /// what a child drew does not depend on how fast the machine is.
    clock: FrameClock,
    /// The host's frames so far.
    pass: u64,
    /// Why the root stopped, once it did, for `run` to exit with.
    stopped: Rc<RefCell<Option<String>>>,
    /// What the last frame showed, so an unchanged frame keeps its drawing.
    drawn: Vec<(Layer, u64)>,
    /// What the root's last frame asked of the window beside its input method's area, which
    /// the window is asked again every frame.
    requests: Requests,
}

/// What a root's frame asks of its window (`frame-done`'s `cursor`, `pointer-locked` and
/// `every-sample`).
#[derive(Clone, Copy, Default)]
struct Requests {
    cursor: CursorIcon,
    pointer_locked: bool,
    every_sample: bool,
}

impl RootShell {
    fn new(app: String, stopped: Rc<RefCell<Option<String>>>) -> Self {
        let mut shown = None;
        let document = beui_host::native::build(|| {
            let drawing = create_signal(None::<GpuDrawing>);
            shown = Some(drawing);
            view! {
                <beui::styled::Root title="be-host">
                    <Drawing @test_id={SHOWN} draw={move || draw_gpu(drawing.get())} />
                </beui::styled::Root>
            }
        });
        Self {
            app,
            document,
            shown: shown.expect("the drawing's signal is made while the document is built"),
            input: Vec::new(),
            root: None,
            time_origin: None,
            origin: None,
            next: None,
            owed: false,
            clock: FrameClock::default(),
            pass: 0,
            stopped,
            drawn: Vec::new(),
            requests: Requests::default(),
        }
    }

    /// The root stopped, or never started, for `why`: it draws nothing again, and be-host exits
    /// saying so once the window closes, which it asks for at the end of the frame.
    fn stopped(&mut self, why: &str) {
        let why = format!("the root {} stopped: {why}", self.app);
        eprintln!("be-host: {why}");
        self.stopped.borrow_mut().get_or_insert(why);
        self.root = None;
        self.next = None;
        self.owed = false;
        self.requests = Requests::default();
    }

    /// Hands the root its windows' input since it was last handed it, in order, a call for each
    /// run of one window's, each input with the frame clock's time it was sampled at, counted
    /// from `origin` as the frames' are. A sample's time is the platform's real instant, so
    /// while drive's clock is stopped it can be later than the frame's. The main window is the one the layer list names as none;
    /// the host opens no other for the root yet.
    fn deliver_input(&mut self, origin: Instant) {
        let input = std::mem::take(&mut self.input);
        let Some(root) = &mut self.root else {
            return;
        };
        for run in input.chunk_by(|(left, ..), (right, ..)| left == right) {
            let window = run[0].0;
            let window = (!window.is_main()).then(|| window.raw());
            let events: Vec<Timed> = run
                .iter()
                .map(|(_, input, time)| Timed {
                    input: input.clone(),
                    time: time.map(|at| nanos(at.saturating_duration_since(origin))),
                })
                .collect();
            if let Err(why) = root.input(window, be_lib_ui_types::input::encode(&events)) {
                return self.stopped(&why);
            }
        }
    }

    /// Asks the root for a frame for `time` and shows its layer list.
    fn frame(&mut self, context: &beui::native::Context, time: Duration) {
        let Some(root) = &mut self.root else {
            return;
        };
        let origin = *self.origin.get_or_insert_with(Instant::now);
        let since = |at: Instant| at.saturating_duration_since(origin);
        self.clock.begin_frame(
            context.next_present().map(since),
            context.refresh_interval(),
            context.last_prepared().map(since),
        );
        self.pass += 1;
        let deadline = origin
            + self
                .clock
                .deadline(self.pass, context.clock_stopped(), since(Instant::now()));
        let request = FrameRequest { time: nanos(time) };
        let done = match root.frame(request, deadline) {
            Ok(done) => done,
            Err(why) => return self.stopped(&why),
        };
        self.next = done.next.map(Duration::from_nanos);
        self.owed = done.owed;
        self.requests = Requests {
            cursor: CursorIcon::from_name(&done.cursor).unwrap_or_default(),
            pointer_locked: done.pointer_locked,
            every_sample: done.every_sample,
        };
        let ime = match done.ime.as_deref().map(beui::ImeArea::decode).transpose() {
            Ok(ime) => ime,
            Err(why) => return self.stopped(&why),
        };
        // The window shows the platform's input method where the root's focused surface edits.
        context.set_ime_area(ime);
        let list = match LayerList::decode(&done.layers) {
            Ok(list) => list,
            Err(why) => return self.stopped(&format!("its layer list cannot be read: {why}")),
        };
        // The host has one window, which the list names as none.
        let mut layers = Vec::new();
        for layer in list
            .windows
            .into_iter()
            .filter(|window| window.window.is_none())
            .flat_map(|window| window.layers)
            .filter(|layer| layer.shown)
        {
            let shown = match layer.surface {
                Surface::Screen { instance, surface } | Surface::Anchored { instance, surface } => {
                    root.surface(instance, surface).cloned()
                }
                Surface::Root(surface) => root.own_surface(surface).cloned(),
                // The host owns no surfaces of its own here.
                Surface::Host(_) => None,
            };
            if let Some(shown) = shown {
                layers.push((layer, shown.generation, shown.texture));
            }
        }
        let drawn: Vec<(Layer, u64)> = layers
            .iter()
            .map(|(layer, generation, _)| (layer.clone(), *generation))
            .collect();
        if drawn == self.drawn {
            return;
        }
        self.drawn = drawn;
        let drawing = (!layers.is_empty()).then(|| {
            beui::native::drawing(ShownLayers {
                layers: layers
                    .into_iter()
                    .map(|(layer, _, texture)| (layer, texture))
                    .collect(),
                quads: Quads::default(),
            })
        });
        let shown = self.shown;
        beui::native::enter(&mut self.document, || shown.set(drawing));
    }
}

impl beui::native::Program for RootShell {
    fn setup(&mut self, setup: &beui::native::Setup) {
        match start(&self.app, setup) {
            Ok(root) => self.root = Some(root),
            Err(why) => self.stopped(&format!("it did not start: {why}")),
        }
    }

    fn renderer_replaced(&mut self, _setup: &beui::native::Setup) {
        // What the root's children drew is on the device it started on, which is gone.
        if self.root.is_some() {
            self.stopped("the window's device was replaced, and the root cannot move to another");
        }
    }

    fn update(&mut self, context: &beui::native::Context, rect: beui::Rect) {
        // The frame clock's time, which stands still while drive's clock is stopped.
        let now = context.now();
        let origin = *self.time_origin.get_or_insert(now);
        let time = now.saturating_duration_since(origin);
        self.deliver_input(origin);
        self.frame(context, time);
        self.document.show(context, rect);
        // After the document, whose own requests (it hears no input, so the default cursor) the
        // root's replace.
        let Requests {
            cursor,
            pointer_locked,
            every_sample,
        } = self.requests;
        context.set_cursor_icon(cursor);
        context.set_pointer_locked(pointer_locked);
        if every_sample {
            context.want_every_sample(WindowId::MAIN);
        }
        if self.stopped.borrow().is_some() {
            context.close_window();
            return;
        }
        match self.next {
            Some(at) if at <= time => context.request_repaint(),
            Some(at) => context.request_repaint_after(at - time),
            None => {}
        }
    }

    /// The root takes all of its windows' input, which it routes to its surfaces itself; beui's
    /// document, which only shows what the root drew, hears none of it.
    fn input(&mut self, window: WindowId, input: &Input, time: Option<Instant>) -> bool {
        self.input.push((window, input.clone(), time));
        true
    }

    /// Only what the root owes now: a frame it wants later, or for an animation, is the repaint
    /// it asked for, which drive tells apart itself.
    fn busy(&self) -> Vec<String> {
        match self.owed {
            true => vec![format!("the root {} owes a frame", self.app)],
            false => Vec::new(),
        }
    }

    fn exiting(&mut self) {
        // Stops the root and every instance it spawned.
        self.root = None;
    }
}

/// Starts `app`'s entry point as the root on the window's device, which it and its staged apps,
/// found as be-host finds every app's, draw on, waking the host whenever it has something to
/// step for.
fn start(app: &str, setup: &beui::native::Setup) -> Result<Root, String> {
    let gpu = setup
        .get::<beui_renderer_wgpu::present::GpuSetup>()
        .ok_or("the window has no GPU renderer, and the root's apps draw on its device")?;
    let apps = be_host_stack::discovery::apps();
    let manifest = apps
        .manifests()
        .iter()
        .find(|manifest| manifest.identity.id == app)
        .ok_or_else(|| {
            let mut why = format!("no staged app has the id {app}");
            for error in apps.errors() {
                why.push_str(&format!("; {error}"));
            }
            why
        })?;
    if !manifest.root {
        return Err(format!(
            "{app} is an app, not a root: its manifest does not say \"root\": true"
        ));
    }
    let host = be_wasm_host::Host::pure()?.with_display(gpu.device.clone(), gpu.queue.clone());
    let module = be_host_runtime::staged_app(app, &manifest.entry_point)?.load(&host)?;
    let root = Root::start(
        module,
        Box::new(be_host_runtime::staged_app),
        gpu.device.clone(),
        gpu.queue.clone(),
    )?;
    let waker = setup.waker.clone();
    root.on_wake(move || waker.wake());
    Ok(root)
}

fn nanos(time: Duration) -> u64 {
    u64::try_from(time.as_nanos()).unwrap_or(u64::MAX)
}

/// The surfaces of the root's frame the window shows, bottom first, each with the texture it
/// names.
struct ShownLayers {
    layers: Vec<(Layer, wgpu::Texture)>,
    quads: Quads,
}

impl Draw for ShownLayers {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let pipelines = Pipelines::of(device, &at);
        let bound: Vec<_> = self
            .layers
            .iter()
            .map(|(layer, texture)| {
                let quad = Quad {
                    clip: at.clip,
                    // An app presents sRGB-encoded values, as it does to the host's own UI.
                    srgb_encoded: true,
                    ..compose::quad(layer, at.pixels_per_point, [0.0; 2])
                };
                (pipelines.bind(texture), quad)
            })
            .collect();
        self.quads.prepare(
            &pipelines,
            queue,
            at.format,
            [at.screen.x, at.screen.y],
            bound.iter().map(|(texture, quad)| (texture, *quad)),
        );
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        self.quads.paint(pass);
    }
}
