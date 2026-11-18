//! be-host with a root guest in place of its own UI (`--root=APP`;
//! plans/host_as_platform_adapter.md, step 4e5c): the staged app `APP`'s entry point runs as the
//! root (be-wasm-root-host's `Root`) on the window's device. Each host frame hands it the
//! window's input since the last, asks it for a frame, and draws the frame's layer list: each
//! screen it names by handle (`Surface::Screen`) is the texture `Root::surface` resolves, drawn
//! as `beui::native::compose::quad` places it; the root's own surfaces are the window's plain
//! background until it draws its own. The host draws on demand: when the root asks for its next
//! frame (`frame-done`'s `next`) or has something to step for (`Root::on_wake`).
//!
//! This is the path step 4g turns on and makes the default, so it runs on be-host's real
//! startup path and window, through beui's runner; only what the window shows comes from the
//! root. beui still catches the window's input (an `Interactive` over the whole window, replayed
//! as the platform's `Input`s with `Replay`), until the host routes by the layers itself (4f).

use std::{
    cell::RefCell,
    error::Error,
    rc::Rc,
    time::{Duration, Instant},
};

use be_lib_layers::{Layer, LayerList, Surface};
use be_wasm_root_api::FrameRequest;
use be_wasm_root_host::Root;
use beui::native::texture_quad::{Pipelines, Quad, Quads};
use beui::native::{Draw, DrawAt, Drawing as GpuDrawing, compose, draw_gpu};
use beui::reactive::*;
use beui::styled::*;
use beui::{Aim, ForwardedInput, Replay};

/// The id the window's input catcher has in the accessibility tree, `#root` to drive.
pub(crate) const CATCHER: &str = "root";

/// How long a frame waits for the root's children when the platform says nothing of when the
/// display presents next.
const UNPACED_DEADLINE: Duration = Duration::from_millis(16);

/// Runs `app`'s entry point as the root in a window opened with `options`.
pub(crate) fn run(app: String, options: beui_host::native::RunOptions) -> Result<(), Box<dyn Error>> {
    beui_host_window::run_with(options, RootShell::new(app))
}

struct RootShell {
    app: String,
    document: beui::native::Document,
    /// What the window shows of the root's last frame.
    shown: Signal<Option<GpuDrawing>>,
    /// What the window's input catcher was given since the last frame.
    caught: Rc<RefCell<Vec<ForwardedInput>>>,
    replay: Replay,
    root: Option<Root>,
    /// Where the frame clock's time the root is asked for starts.
    origin: Option<Instant>,
    /// When the root wants its next frame, in its time, as its last frame said.
    next: Option<Duration>,
    /// What the last frame showed, so an unchanged frame keeps its drawing.
    drawn: Vec<(Layer, u64)>,
}

impl RootShell {
    fn new(app: String) -> Self {
        let caught: Rc<RefCell<Vec<ForwardedInput>>> = Rc::default();
        let mut shown = None;
        let catching = Rc::clone(&caught);
        let document = beui_host::native::build(|| {
            let drawing = create_signal(None::<GpuDrawing>);
            shown = Some(drawing);
            view! {
                <beui::styled::Root title="be-host">
                    <Interactive
                        focusable=true
                        @test_id={CATCHER}
                        on_forward={move |input: ForwardedInput| catching.borrow_mut().push(input)}
                    >
                        <Drawing draw={move || draw_gpu(drawing.get())} />
                    </Interactive>
                </beui::styled::Root>
            }
        });
        Self {
            app,
            document,
            shown: shown.expect("the drawing's signal is made while the document is built"),
            caught,
            replay: Replay::default(),
            root: None,
            origin: None,
            next: None,
            drawn: Vec::new(),
        }
    }

    /// Says why the root stopped, which then draws nothing again.
    fn stopped(&mut self, why: &str) {
        eprintln!("be-host: the root {} stopped: {why}", self.app);
        self.root = None;
        self.next = None;
    }

    /// Hands the root the input caught since the last frame, in the window's points.
    fn deliver_input(&mut self) {
        let caught = std::mem::take(&mut *self.caught.borrow_mut());
        let events: Vec<beui::Input> = caught
            .iter()
            .flat_map(|input| self.replay.replay(input, 1.0, |_, _| Aim::Own))
            .map(|replayed| replayed.input)
            .collect();
        let Some(root) = &mut self.root else {
            return;
        };
        if events.is_empty() {
            return;
        }
        if let Err(why) = root.input(None, be_lib_ui_types::input::encode(&events)) {
            self.stopped(&why);
        }
    }

    /// Asks the root for a frame for `time` and shows its layer list.
    fn frame(&mut self, context: &beui::native::Context, time: Duration) {
        let Some(root) = &mut self.root else {
            return;
        };
        let now = Instant::now();
        let deadline = context
            .next_present()
            .filter(|at| *at > now)
            .unwrap_or(now + UNPACED_DEADLINE);
        let request = FrameRequest {
            time: nanos(time),
        };
        let done = match root.frame(request, deadline) {
            Ok(done) => done,
            Err(why) => return self.stopped(&why),
        };
        self.next = done.next.map(Duration::from_nanos);
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
            match layer.surface {
                Surface::Screen { instance, surface } | Surface::Anchored { instance, surface } => {
                    if let Some(shown) = root.surface(instance, surface) {
                        layers.push((layer, shown.generation, shown.texture.clone()));
                    }
                }
                // The root's own surfaces are the window's background until it draws its own, and
                // the host owns no surfaces of its own here.
                Surface::Root(_) | Surface::Host(_) => {}
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
            Err(why) => eprintln!("be-host: the root {} did not start: {why}", self.app),
        }
    }

    fn renderer_replaced(&mut self, _setup: &beui::native::Setup) {
        // What the root's children drew is on the device it started on, which is gone.
        if self.root.is_some() {
            self.stopped("the window's device was replaced, and the root cannot move to another");
        }
    }

    fn update(&mut self, context: &beui::native::Context, rect: beui::Rect) {
        let now = context.now();
        let time = now.saturating_duration_since(*self.origin.get_or_insert(now));
        self.deliver_input();
        self.frame(context, time);
        self.document.show(context, rect);
        // What the window caught this frame reaches the root with the next.
        if !self.caught.borrow().is_empty() {
            context.request_repaint();
        }
        match self.next {
            Some(at) if at <= time => context.request_repaint(),
            Some(at) => context.request_repaint_after(at - time),
            None => {}
        }
    }

    fn busy(&self) -> Vec<String> {
        match self.next {
            Some(_) => vec![format!("the root {} wants another frame", self.app)],
            None => Vec::new(),
        }
    }

    fn exiting(&mut self) {
        // Stops the root and every instance it spawned.
        self.root = None;
    }
}

/// Starts `app`'s entry point as the root on the window's device, its staged apps found as
/// be-host finds every app's, and waking the host whenever it has something to step for.
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
    let module = be_host_runtime::staged_app(app, &manifest.entry_point)?
        .load(&be_wasm_host::Host::pure()?)?;
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
