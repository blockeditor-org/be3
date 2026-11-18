//! beui on a root guest's own surfaces (plans/host_as_platform_adapter.md, step 4g2): a beui
//! document, run by beui's runner as any beui program is, drawn with beui's wgpu renderer over
//! `be3:gpu` into one of be-sdk-root's `OwnSurface`s, which the root names in its layer list as
//! `Surface::Root`. The pixels never leave the host's device.
//!
//! The document hears what the root's router lands on the surface (be-sdk-root's
//! `Embedding::input` and `drew`), in its points, and draws its text in the fonts its host gives
//! the root (`be3:root`'s `host.fonts`). It is one window of a beui program of its own, so a root
//! that draws several surfaces with beui runs one `BeuiSurface` for each.
//!
//! A root links it as an app links be-sdk-beui: against the app libraries, whose beui library
//! holds beui, its runner, its renderer and FreeType
//! (decisions/a_root_links_beui_as_an_app_does.md). be-sdk-root itself stays free of any GUI
//! framework; this is the beui half beside it.

use std::{cell::RefCell, rc::Rc, time::Duration};

use be_lib_geometry::Pos2;
use be_lib_layers::{Layer, LayerId, Surface};
use be_sdk_root::{Delivery, OwnSurface};
use be_wasm_root_api::{FontFace, FontRole};
use beui_core::context::{Context, RendererInfo, WindowOutput};
use beui_core::painting::{Painting, Punches};
use beui_core::program::{Program, SafeArea, Setup, Waker};
use beui_font_freetype::{FontBytes, FontData, FontLibrary, FontSources, FreetypeFonts};
use beui_renderer_wgpu::{Repaint, Resources};
use beui_runner::renderer::{Loaded, Renderer};
use beui_runner::{Incoming, Launch, PlatformServices, RunOptions, Runner};
use beui_types::geometry::Region;
use beui_types::{Color32, Rect, Vec2, WindowId, vec2};

/// A beui document drawn on one of the root's own surfaces.
pub struct BeuiSurface {
    runner: Runner,
    drawn: Rc<RefCell<Drawn>>,
    raster: f32,
    /// When the document wants its next frame, after the last: none until something wakes it.
    wait: Option<Duration>,
}

/// What the renderer and the surface share.
struct Drawn {
    surface: OwnSurface,
    /// Why the last frame could not be drawn, until it is told.
    failure: Option<String>,
}

/// The surface's window, which asks the platform for nothing it can give: the root has no
/// clipboard, files or keyboard of its own to offer yet.
struct Window;

impl PlatformServices for Window {}

/// A program that shows one document, filling its window.
struct Showing(beui_core::document::Document);

impl Program for Showing {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.show(context, rect);
    }
}

impl BeuiSurface {
    /// Shows the document `root` builds on `surface`, at `raster` pixels a point, its text in `fonts`.
    /// `wake` is called on any thread when the document wants a frame, such as when one of its
    /// async memos is woken: the root wakes itself, and so its host.
    pub fn new(
        surface: OwnSurface,
        raster: f32,
        fonts: &[FontFace],
        wake: impl Fn() + Send + Sync + 'static,
        root: impl FnOnce() -> beui_host::primitives::Root,
    ) -> Result<Self, String> {
        let document = beui_host::native::build(root);
        let (width, height) = surface.size();
        let drawn = Rc::new(RefCell::new(Drawn {
            surface,
            failure: None,
        }));
        let context = Context::new(FreetypeFonts::new(FontLibrary::new(sources(fonts))));
        let mut runner = Runner::new(Launch {
            options: RunOptions::default(),
            context,
            program: Box::new(Showing(document)),
            layers: Vec::new(),
        });
        runner.reports().resized(width, height);
        runner.reports().scaled(raster);
        runner
            .start(
                Loaded {
                    renderer: Box::new(OwnRenderer {
                        drawn: Rc::clone(&drawn),
                        gpu: None,
                        renderer: None,
                        pending: None,
                        stale: true,
                    }),
                    fonts: None,
                },
                Setup::new(Waker::new(wake)),
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            runner,
            drawn,
            raster,
            wait: Some(Duration::ZERO),
        })
    }

    /// The surface's id, which its layers name as `Surface::Root`.
    pub fn id(&self) -> u32 {
        self.drawn.borrow().surface.id()
    }

    /// Makes the surface `size` pixels at `raster` pixels a point: the document lays out
    /// anew for it at its next frame.
    pub fn resize(&mut self, size: (u32, u32), raster: f32) {
        self.raster = raster;
        self.runner.reports().resized(size.0, size.1);
        self.runner.reports().scaled(raster);
        self.wait = Some(Duration::ZERO);
    }

    /// Hands the document what the root's router delivered to this surface, ignoring what is
    /// for any other: each input in the surface's pixels, as the router answers it, becomes
    /// the document's in its points.
    pub fn input(&mut self, deliveries: &[Delivery]) {
        let id = self.id();
        for delivery in deliveries {
            if delivery.surface != Surface::Root(id) {
                continue;
            }
            let input = delivery.input.clone().scaled(1.0 / self.raster);
            self.runner.push(Incoming::points(input));
            self.wait = Some(Duration::ZERO);
        }
    }

    /// Whether the document wants a frame now: it heard input, was resized, asked for a
    /// repaint or animates, or has not drawn yet.
    pub fn wants_frame(&self) -> bool {
        self.wait == Some(Duration::ZERO)
    }

    /// How long after its last frame the document wants its next: none until it hears input
    /// or something wakes it.
    pub fn wait(&self) -> Option<Duration> {
        self.wait
    }

    /// Runs a frame of the document and, if it changed, draws it and presents the surface.
    /// Says why it could not draw.
    pub fn frame(&mut self) -> Result<(), String> {
        let frame = self.runner.frame(&mut Window, SafeArea::default());
        self.runner.present();
        if let Some(failure) = self.runner.failure() {
            return Err(failure.to_owned());
        }
        self.wait = frame.and_then(|frame| frame.wait());
        match self.drawn.borrow_mut().failure.take() {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }

    /// The layer `id` showing the whole surface with its top left at `origin` in its window's
    /// points. It takes input over all of it.
    pub fn layer(&self, id: LayerId, origin: Pos2) -> Layer {
        self.drawn.borrow().surface.layer(id, origin, self.raster)
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

/// Draws the document's window into the surface: beui's wgpu renderer on the device `be3:gpu`
/// gives the root, as be-sdk-beui-platform draws an app's screens.
struct OwnRenderer {
    drawn: Rc<RefCell<Drawn>>,
    gpu: Option<(wgpu::Device, wgpu::Queue, Resources)>,
    renderer: Option<beui_renderer_wgpu::Renderer>,
    /// The output the next present draws.
    pending: Option<(WindowOutput, f32, Color32)>,
    /// Whether the surface needs drawing whole even if the output did not change: it was made
    /// or resized.
    stale: bool,
}

impl OwnRenderer {
    fn draw(&mut self, output: &WindowOutput, scale: f32, background: Color32) -> Result<(), String> {
        if self.gpu.is_none() {
            let (device, queue) = be_wasm_gpu_guest::request_device(&wgpu::DeviceDescriptor {
                label: Some("root device"),
                required_limits: be_wasm_gpu_guest::limits(),
                ..Default::default()
            })?;
            // The surface holds sRGB-encoded values in a plain format, as an app's screens do.
            let resources = Resources::new(&device, wgpu::TextureFormat::Rgba8Unorm);
            self.gpu = Some((device, queue, resources));
        }
        let Some((device, queue, resources)) = &self.gpu else {
            return Ok(());
        };
        let drawn = self.drawn.borrow();
        let (width, height) = drawn.surface.size();
        let texture = be_wasm_gpu_guest::texture(drawn.surface.acquire()?)?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let renderer = self
            .renderer
            .get_or_insert_with(|| beui_renderer_wgpu::Renderer::new(resources));
        let screen = vec2(width as f32, height as f32);
        // The texture may hold any older frame, so the whole of it is drawn.
        let whole = Repaint::Region {
            region: Region::from(Rect::from_min_size(Pos2::ZERO, screen / scale)),
            background,
        };
        if let Repaint::Everything = renderer.prepare(device, queue, output, screen, scale, whole) {
            renderer.prepare(device, queue, output, screen, scale, whole);
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer.render(
            device,
            queue,
            &mut encoder,
            &view,
            (width, height),
            wgpu::LoadOp::Load,
        );
        queue.submit([encoder.finish()]);
        drawn.surface.present();
        Ok(())
    }
}

impl Renderer for OwnRenderer {
    fn info(&self) -> RendererInfo {
        RendererInfo { rows: Vec::new() }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.drawn.borrow_mut().surface.resize((width, height));
        self.stale = true;
    }

    fn physical(&self) -> Option<Vec2> {
        let (width, height) = self.drawn.borrow().surface.size();
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
                Err(failure) => self.drawn.borrow_mut().failure = Some(failure),
            }
        }
        false
    }
}

/// beui's font sources of the faces the host gave.
fn sources(fonts: &[FontFace]) -> FontSources {
    let mut sources = FontSources::default();
    for face in fonts {
        let list = match face.role {
            FontRole::Text => &mut sources.proportional,
            FontRole::Monospace => &mut sources.monospace,
            FontRole::Fallback => &mut sources.fallback,
            FontRole::Icons => &mut sources.icons,
        };
        list.push(FontData::new(
            FontBytes::Shared(std::sync::Arc::from(face.data.as_slice())),
            face.index,
        ));
    }
    sources
}
