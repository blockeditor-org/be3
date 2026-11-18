//! beui on a root guest's own surfaces (plans/host_as_platform_adapter.md, step 4g2): a beui
//! document, run by beui's runner as any beui program is, drawn with beui's wgpu renderer over
//! `be3:gpu` into one of be-sdk-root's `OwnSurface`s, which the root names in its layer list as
//! `Surface::Root`. The pixels never leave the host's device.
//!
//! `BeuiRoot` is what every surface of one root shares: the device `be3:gpu` gives it, the
//! fonts its host gave (`be3:root`'s `host.fonts`) and the frame clock its host hands each frame.
//! `BeuiSurface` is one document on one surface, one window of a beui program of its own: it
//! hears what the root's router lands on the surface (be-sdk-root's `Embedding::input` and
//! `drew`), in its points, and draws when it heard something, was woken (an async memo's wake),
//! or asked for a frame by now (an animation, a repaint after a delay, a timer).
//!
//! A document that shows only overlays over a clear window, as the root's toasts do
//! (plans/host_as_platform_adapter.md, step 4g3), is hit only where they are: its layer takes
//! `BeuiSurface::overlay_area`, the rects its overlays take the pointer in, as its hit area,
//! and what lies under it takes the rest.
//!
//! A root links it as an app links be-sdk-beui: against the app libraries, whose beui library
//! holds beui, its runner, its renderer and FreeType
//! (decisions/a_root_links_beui_as_an_app_does.md). be-sdk-root itself stays free of any GUI
//! framework; this is the beui half beside it.

use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use be_lib_geometry::Pos2;
use be_lib_layers::{Area, Layer, LayerId, Surface};
use be_lib_ui_types::Fonts;
use be_sdk_root::{Delivery, OwnSurface};
use beui_core::context::{Context, RendererInfo, WindowOutput};
use beui_core::painting::{Painting, Punches};
use beui_core::program::{Program, SafeArea, Setup, Waker};
use beui_font_freetype::{FontBytes, FontData, FontLibrary, FontSources, FreetypeFonts};
use beui_renderer_wgpu::{Repaint, Resources};
use beui_runner::renderer::{Loaded, Renderer};
use beui_runner::{Incoming, Launch, PlatformServices, RunOptions, Runner};
use beui_types::geometry::Region;
use beui_types::{Color32, Rect, Vec2, WindowId, vec2};

/// What the surfaces of one root drawn with beui share.
#[derive(Clone)]
pub struct BeuiRoot(Rc<Shared>);

struct Shared {
    device: wgpu::Device,
    queue: wgpu::Queue,
    fonts: FontLibrary,
    /// The instant the frame clock's zero is at, so beui's clock reads the host's frame time.
    epoch: Instant,
    /// The frame clock's time and the wall clock at it, as the host handed them last.
    clock: Cell<(Duration, Duration)>,
}

impl BeuiRoot {
    /// The device `be3:gpu` gives the root, and the fonts its host gave (`host.fonts`'s
    /// answer: be-sdk-api's `Fonts`, encoded), at the frame clock's `time` and the host's
    /// `wall_clock`, as `frame-request` carries them.
    pub fn new(fonts: &[u8], time: Duration, wall_clock: Duration) -> Result<Self, String> {
        let fonts: Fonts = be_wasm_ops::decode(fonts).ok_or("the host's fonts cannot be read")?;

        // TODO: a character none of these has is drawn as nothing: the host has no call yet that
        // answers a fallback face for it from the system's fonts, as an app's
        // `MissingCharacters` would be (plans/host_as_platform_adapter.md, 4g2's leftovers).
        let fonts = FontLibrary::new(FontSources::of_faces(fonts.faces.into_iter().map(|face| {
            let data = FontData::new(FontBytes::Shared(Arc::from(face.data)), face.index);
            (face.role, data)
        })));
        let (device, queue) = be_wasm_gpu_guest::request_device(&wgpu::DeviceDescriptor {
            label: Some("root device"),
            required_limits: be_wasm_gpu_guest::limits(),
            ..Default::default()
        })?;
        // Real time once, for where the frame clock's zero lies; beui reads only the host's
        // times after.
        let real = Instant::now();
        Ok(Self(Rc::new(Shared {
            device,
            queue,
            fonts,
            epoch: real.checked_sub(time).unwrap_or(real),
            clock: Cell::new((time, wall_clock)),
        })))
    }

    /// The host's clocks for the frame being drawn: the frame clock's `time` and the wall
    /// clock at it, which every document reads as its now.
    pub fn set_clock(&self, time: Duration, wall_clock: Duration) {
        self.0.clock.set((time, wall_clock));
    }

    /// The frame clock's time the host handed last.
    pub fn time(&self) -> Duration {
        self.0.clock.get().0
    }

    /// Pins beui's clocks for `context` to the host's.
    fn pin(&self, context: &Context) {
        let (time, wall_clock) = self.0.clock.get();
        let now = self.0.epoch + time;
        context.set_clock(now);
        beui_core::timer::pin_wall_clock(Some((now, wall_clock)));
    }
}

/// A beui document drawn on one of the root's own surfaces.
pub struct BeuiSurface {
    root: BeuiRoot,
    runner: Runner,
    drawn: Rc<Drawn>,
    raster: f32,
    /// Set by the document's waker: it wants a frame now.
    woken: Arc<AtomicBool>,
    /// Wakes the root, and so its host, for a frame of the document.
    wake: Arc<dyn Fn() + Send + Sync>,
    /// The frame clock's time the document wants its next frame at: none until it hears
    /// input or is woken.
    due: Option<Duration>,
    /// Where its overlays took the pointer in its last frame, in its points.
    overlays: Rc<RefCell<Vec<Rect>>>,
}

/// What the renderer and the surface share.
struct Drawn {
    surface: std::cell::RefCell<OwnSurface>,
    /// Why the last frame could not be drawn, until it is told.
    failure: std::cell::RefCell<Option<String>>,
}

/// The surface's window, which asks the platform for nothing it can give: the root has no
/// clipboard, files or keyboard of its own to offer yet.
struct Window;

impl PlatformServices for Window {}

/// A program that shows one document, filling its window.
struct Showing {
    document: beui_core::document::Document,
    /// Where the document's overlays took the pointer in its last frame, in its points.
    overlays: Rc<RefCell<Vec<Rect>>>,
}

impl Program for Showing {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
        *self.overlays.borrow_mut() = self.document.overlay_rects(WindowId::MAIN);
    }
}

impl BeuiSurface {
    /// Shows the document `build` makes on `surface`, at `raster` pixels a point. `wake` is
    /// called on any thread when the document wants a frame, such as when one of its async memos
    /// is woken: the root wakes itself, and so its host. Its first frame is due at once.
    pub fn new(
        root: &BeuiRoot,
        surface: OwnSurface,
        raster: f32,
        wake: impl Fn() + Send + Sync + 'static,
        build: impl FnOnce() -> beui_host::primitives::Root,
    ) -> Result<Self, String> {
        let (width, height) = surface.size();
        let drawn = Rc::new(Drawn {
            surface: std::cell::RefCell::new(surface),
            failure: std::cell::RefCell::default(),
        });
        let context = Context::new(FreetypeFonts::new(root.0.fonts.clone()));
        // Built at the host's time, so a timer the document starts counts from it.
        root.pin(&context);
        let document = beui_host::native::build(build);
        let overlays = Rc::<RefCell<Vec<Rect>>>::default();
        let mut runner = Runner::new(Launch {
            options: RunOptions::default(),
            context,
            program: Box::new(Showing {
                document,
                overlays: Rc::clone(&overlays),
            }),
            layers: Vec::new(),
        });
        runner.reports().resized(width, height);
        runner.reports().scaled(raster);
        let woken = Arc::new(AtomicBool::new(false));
        let waking = Arc::clone(&woken);
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let waker = Arc::clone(&wake);
        runner
            .start(
                Loaded {
                    renderer: Box::new(OwnRenderer {
                        root: root.clone(),
                        drawn: Rc::clone(&drawn),
                        renderer: None,
                        pending: None,
                        stale: true,
                    }),
                    fonts: None,
                },
                Setup::new(Waker::new(move || {
                    waking.store(true, Ordering::Release);
                    waker();
                })),
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            root: root.clone(),
            runner,
            drawn,
            raster,
            woken,
            wake,
            due: Some(Duration::ZERO),
            overlays,
        })
    }

    /// The surface's id, which its layers name as `Surface::Root`.
    pub fn id(&self) -> u32 {
        self.drawn.surface.borrow().id()
    }

    /// Makes the surface `size` pixels at `raster` pixels a point: it is configured afresh now,
    /// showing its last frame until the document draws it anew at its next frame, which is due
    /// at once.
    pub fn resize(&mut self, size: (u32, u32), raster: f32) {
        self.raster = raster;
        self.drawn.surface.borrow_mut().resize(size);
        self.runner.reports().resized(size.0, size.1);
        self.runner.reports().scaled(raster);
        self.due = Some(Duration::ZERO);
    }

    /// Hands the document what the root's router delivered to this surface, ignoring what is
    /// for any other: each input in the surface's pixels, as the router answers it, becomes
    /// the document's in its points. Its next frame is due at once.
    pub fn input(&mut self, deliveries: &[Delivery]) {
        let id = self.id();
        for delivery in deliveries {
            if delivery.surface != Surface::Root(id) {
                continue;
            }
            let input = delivery.input.clone().scaled(1.0 / self.raster);
            self.runner.push(Incoming::points(input));
            self.due = Some(Duration::ZERO);
        }
    }

    /// The frame clock's time the document wants its next frame at: at once once it heard
    /// input, was resized or woken; after a while while it animates, waits to repaint or runs a
    /// timer; none while it waits for nothing.
    pub fn next(&self) -> Option<Duration> {
        match self.woken.load(Ordering::Acquire) {
            true => Some(Duration::ZERO),
            false => self.due,
        }
    }

    /// Whether the document wants a frame by the root's frame clock's time (`BeuiRoot::time`).
    pub fn wants_frame(&self) -> bool {
        self.next().is_some_and(|due| due <= self.root.time())
    }

    /// Runs a frame of the document at the root's frame clock's time, if it wants one by then,
    /// and if it changed, draws it and presents the surface. Says why it could not draw.
    pub fn frame(&mut self) -> Result<(), String> {
        if !self.wants_frame() {
            return Ok(());
        }
        self.woken.store(false, Ordering::Release);
        self.root.pin(self.runner.context());
        let frame = self.runner.frame(&mut Window, SafeArea::default());
        self.runner.present();
        if let Some(failure) = self.runner.failure() {
            return Err(failure.to_owned());
        }
        let time = self.root.time();
        self.due = frame.and_then(|frame| frame.wait()).map(|wait| time + wait);
        match self.drawn.failure.borrow_mut().take() {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }

    /// Runs `change` in the document, as its own code runs, so it may set the signals the
    /// document reads: what the root tells it from outside, such as its host's notices. Its
    /// next frame is due at once, and the root is woken for it.
    pub fn enter<R>(&mut self, change: impl FnOnce() -> R) -> R {
        let program: &mut dyn Any = self.runner.program_mut();
        let showing = program
            .downcast_mut::<Showing>()
            .expect("a root's surface runs only the program that shows its document");
        let changed = beui_core::current::enter(&mut showing.document, change);
        self.due = Some(Duration::ZERO);
        self.woken.store(true, Ordering::Release);
        (self.wake)();
        changed
    }

    /// Where the document's overlays (its toasts, popups and dialogs, with any scrim one lays
    /// over the rest) took the pointer in its last frame, in the surface's pixels: as a layer's
    /// hit area (be-lib-layers' `LayerInput::hit`), what shows only overlays over a clear
    /// document is hit only there, and what lies under it takes the rest. Nowhere before it drew
    /// or while it shows none.
    pub fn overlay_area(&self) -> Area {
        let rects: Vec<Rect> = self
            .overlays
            .borrow()
            .iter()
            .map(|rect| {
                let pixels = |at: Pos2| Pos2::new(at.x * self.raster, at.y * self.raster);
                Rect::from_min_max(pixels(rect.min), pixels(rect.max))
            })
            .collect();
        match rects.is_empty() {
            true => Area::Nowhere,
            false => Area::Region(rects),
        }
    }

    /// The layer `id` showing the whole surface with its top left at `origin` in its window's
    /// points. It takes input over all of it.
    pub fn layer(&self, id: LayerId, origin: Pos2) -> Layer {
        self.drawn.surface.borrow().layer(id, origin, self.raster)
    }

    /// What the document painted in its last frame, in the surface's pixels, as beui-paint-format
    /// keeps a painting: what the host's tests compare. None before it drew.
    pub fn painting(&self) -> Option<Painting> {
        let output = self.runner.output()?.window(WindowId::MAIN)?;
        beui_core::painting::painting(
            output,
            output.size,
            output.pixels_per_point,
            output.window.appearance.clear_color,
            Punches::Erase,
        )
        .ok()
    }
}

/// Draws the document's window into the surface: beui's wgpu renderer on the root's device, as
/// be-sdk-beui-platform draws an app's screens.
struct OwnRenderer {
    root: BeuiRoot,
    drawn: Rc<Drawn>,
    renderer: Option<beui_renderer_wgpu::Renderer>,
    /// The output the next present draws.
    pending: Option<(WindowOutput, f32, Color32)>,
    /// Whether the surface needs drawing whole even if the output did not change: it was made
    /// or resized.
    stale: bool,
}

/// The format of a root's own surfaces: sRGB-encoded values in a plain format, as an app's
/// screens hold them.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl OwnRenderer {
    fn draw(
        &mut self,
        output: &WindowOutput,
        scale: f32,
        background: Color32,
    ) -> Result<(), String> {
        let Shared { device, queue, .. } = &*self.root.0;
        let surface = self.drawn.surface.borrow();
        let (width, height) = surface.size();
        let texture = be_wasm_gpu_guest::texture(surface.acquire()?)?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let renderer = self.renderer.get_or_insert_with(|| {
            beui_renderer_wgpu::Renderer::new(&Resources::shared(device, FORMAT))
        });
        let screen = vec2(width as f32, height as f32);
        // The texture may hold any older frame, so the whole of it is drawn.
        let whole = Repaint::Region {
            region: Region::from(Rect::from_min_size(Pos2::ZERO, screen / scale)),
            background,
        };
        renderer.draw_onto(
            device,
            queue,
            output,
            (screen, scale),
            (whole, whole),
            &view,
            (width, height),
        );
        surface.present();
        Ok(())
    }
}

impl Renderer for OwnRenderer {
    fn info(&self) -> RendererInfo {
        RendererInfo { rows: Vec::new() }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.drawn.surface.borrow_mut().resize((width, height));
        self.stale = true;
    }

    fn physical(&self) -> Option<Vec2> {
        let (width, height) = self.drawn.surface.borrow().size();
        Some(vec2(width as f32, height as f32))
    }

    fn prepare(&mut self, output: &WindowOutput, scale: f32, background: Color32) -> bool {
        if !output.changed && !self.stale {
            return false;
        }
        self.pending = Some((output.clone(), scale, background));
        true
    }

    fn present(&mut self, _background: Color32) -> bool {
        if let Some((output, scale, background)) = self.pending.take() {
            match self.draw(&output, scale, background) {
                Ok(()) => self.stale = false,
                Err(failure) => *self.drawn.failure.borrow_mut() = Some(failure),
            }
        }
        false
    }
}
