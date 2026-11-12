use std::error::Error;
use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use beui_core::context::{Context, RendererInfo, WindowOutput};
use beui_core::font::FontBackend;
use beui_core::program::Setup;
use beui_types::Color32;
use beui_types::Vec2;

#[derive(Clone)]
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub trait WindowHandle: HasWindowHandle + HasDisplayHandle + Send + Sync {}

impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync> WindowHandle for T {}

/// Platforms draw through this and name no renderer crate. beui-host hands the
/// runner a loader returning `Loaded`; the platform attaches and detaches the
/// renderer through `Runner::rendering`.
pub trait Renderer {
    fn info(&self) -> RendererInfo;

    fn provide(&self, _setup: &mut Setup) {}

    fn recover(&mut self) -> Result<bool, Box<dyn Error>> {
        Ok(false)
    }

    fn attach(
        &mut self,
        _window: Arc<dyn WindowHandle>,
        _width: u32,
        _height: u32,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn detach(&mut self) {}

    fn resize(&mut self, width: u32, height: u32);

    fn physical(&self) -> Option<Vec2>;

    fn prepare(&mut self, output: &WindowOutput, scale: f32, background: Color32) -> bool;

    /// Takes an unseen frame (`Runner::set_occluded`). The wgpu target keeps only
    /// what the frame damaged and draws the union of every noted frame's damage the
    /// next time it draws; the default prepares as usual.
    fn note(&mut self, output: &WindowOutput, scale: f32, background: Color32) {
        self.prepare(output, scale, background);
    }

    fn present(&mut self, background: Color32) -> bool;

    fn set_vsync(&mut self, _vsync: bool) {}

    fn capture(&mut self) -> Option<Capture> {
        None
    }
}

pub struct NoRenderer {
    size: Vec2,
}

impl NoRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            size: Vec2::new(width as f32, height as f32),
        }
    }
}

impl Renderer for NoRenderer {
    fn info(&self) -> RendererInfo {
        RendererInfo {
            rows: vec![(
                "Renderer",
                "none: frames are painted but not drawn".to_owned(),
            )],
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = Vec2::new(width as f32, height as f32);
    }

    fn physical(&self) -> Option<Vec2> {
        Some(self.size)
    }

    fn prepare(&mut self, _output: &WindowOutput, _scale: f32, _background: Color32) -> bool {
        true
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

pub struct Loaded {
    pub renderer: Box<dyn Renderer>,
    pub fonts: Option<Box<dyn FontBackend>>,
}

pub struct Rendering {
    renderer: Box<dyn Renderer>,
    attached: bool,
    vsync: bool,
}

impl Rendering {
    pub fn new(context: &Context, loaded: Loaded) -> Self {
        let Loaded { renderer, fonts } = loaded;
        if let Some(fonts) = fonts {
            context.replace_fonts(fonts);
        }
        context.set_renderer_info(renderer.info());
        Self {
            renderer,
            attached: false,
            vsync: true,
        }
    }

    pub fn provide(&self, setup: &mut Setup) {
        self.renderer.provide(setup);
    }

    pub fn recover(&mut self) -> Result<bool, Box<dyn Error>> {
        self.renderer.recover()
    }

    pub fn attach(
        &mut self,
        window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn Error>> {
        self.attached = true;
        self.renderer.attach(window, width, height)
    }

    pub fn attached(&self) -> bool {
        self.attached
    }

    pub fn detach(&mut self) {
        self.attached = false;
        self.renderer.detach();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    pub fn physical(&self) -> Option<Vec2> {
        self.renderer.physical()
    }

    pub fn prepare(&mut self, output: &WindowOutput, scale: f32, background: Color32) -> bool {
        self.renderer.prepare(output, scale, background)
    }

    pub fn note(&mut self, output: &WindowOutput, scale: f32, background: Color32) {
        self.renderer.note(output, scale, background);
    }

    pub fn present(&mut self, background: Color32) -> bool {
        self.renderer.present(background)
    }

    pub fn capture(&mut self) -> Option<Capture> {
        self.renderer.capture()
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        if vsync != std::mem::replace(&mut self.vsync, vsync) {
            self.renderer.set_vsync(vsync);
        }
    }
}

/// Lets another thread take the window's surface away from a renderer running on
/// the platform's thread, as Android's `surfaceDestroyed` does: the surface must
/// be out of use when it returns. `revoke` waits only for a draw in progress,
/// never for a whole frame, so a long step of the program cannot hold it up. A
/// draw in progress may itself be waiting for the swapchain's next image, so
/// `revoke` can wait about a display refresh.
///
/// The renderer is detached later, on the platform's thread, so its swapchain
/// can outlive the surface; a platform whose graphics API minds that (EGL)
/// waits briefly for the detach before it revokes.
///
/// Each surface the platform is handed is a lease, numbered by `current` on the
/// thread that hands it over and given to `Leased::attaching` before the renderer
/// is attached to it. Once `revoke` ends that lease, the renderer does not touch
/// the surface again: what would draw to it does nothing until the platform
/// detaches the renderer and attaches the next surface.
#[derive(Default)]
pub struct SurfaceLease {
    current: std::sync::Mutex<u64>,
}

impl SurfaceLease {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// The lease of the surface the platform is being handed now.
    pub fn current(&self) -> u64 {
        *self.lock()
    }

    /// Ends the current surface's lease. Returns once nothing draws to it.
    pub fn revoke(&self) {
        *self.lock() += 1;
    }

    /// Keeps the surface of lease `handed` from being revoked while the guard
    /// lives, as while renderers load and probe it; `None` once it has been.
    /// Loading at a cold start opens the GPU, and `revoke` (Android's
    /// `surfaceDestroyed`, on the UI thread) waits that long if it comes then.
    pub fn hold(&self, handed: u64) -> Option<SurfaceHeld<'_>> {
        let current = self.lock();
        (*current == handed).then_some(SurfaceHeld { _held: current })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, u64> {
        self.current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// A surface kept from being revoked (`SurfaceLease::hold`) while it lives.
pub struct SurfaceHeld<'a> {
    _held: std::sync::MutexGuard<'a, u64>,
}

/// A renderer that draws to its window only while the window's `SurfaceLease`
/// lasts.
pub struct Leased {
    inner: Box<dyn Renderer>,
    lease: Arc<SurfaceLease>,
    attaching: Arc<std::sync::atomic::AtomicU64>,
    held: Option<u64>,
}

/// Names the lease of the surface the platform attaches next (`Leased`).
#[derive(Clone)]
pub struct Attaching(Arc<std::sync::atomic::AtomicU64>);

impl Attaching {
    pub fn set(&self, lease: u64) {
        self.0.store(lease, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Leased {
    pub fn new(inner: Box<dyn Renderer>, lease: Arc<SurfaceLease>) -> (Self, Attaching) {
        let attaching = Arc::new(std::sync::atomic::AtomicU64::new(lease.current()));
        let leased = Self {
            inner,
            lease,
            attaching: attaching.clone(),
            held: None,
        };
        (leased, Attaching(attaching))
    }

    /// Runs `draw` with the surface while its lease lasts.
    fn with_surface<R>(&mut self, gone: R, draw: impl FnOnce(&mut dyn Renderer) -> R) -> R {
        let current = self.lease.lock();
        match self.held {
            Some(held) if held == *current => draw(&mut *self.inner),
            _ => gone,
        }
    }

    /// Runs `change` with the surface while its lease lasts, or straight away
    /// while there is no surface to touch.
    fn configure(&mut self, change: impl FnOnce(&mut dyn Renderer)) {
        match self.held {
            Some(_) => self.with_surface((), change),
            None => change(&mut *self.inner),
        }
    }
}

impl Renderer for Leased {
    fn info(&self) -> RendererInfo {
        self.inner.info()
    }

    fn provide(&self, setup: &mut Setup) {
        self.inner.provide(setup);
    }

    /// Recovering may make a surface from the window, so it waits for a surface
    /// to be attached.
    fn recover(&mut self) -> Result<bool, Box<dyn Error>> {
        self.with_surface(Ok(false), |inner| inner.recover())
    }

    fn attach(
        &mut self,
        window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn Error>> {
        let lease = self.attaching.load(std::sync::atomic::Ordering::Relaxed);
        self.held = Some(lease);
        self.with_surface(Ok(()), |inner| inner.attach(window, width, height))
    }

    fn detach(&mut self) {
        self.held = None;
        self.inner.detach();
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.configure(|inner| inner.resize(width, height));
    }

    fn physical(&self) -> Option<Vec2> {
        self.inner.physical()
    }

    fn prepare(&mut self, output: &WindowOutput, scale: f32, background: Color32) -> bool {
        self.with_surface(false, |inner| inner.prepare(output, scale, background))
    }

    fn note(&mut self, output: &WindowOutput, scale: f32, background: Color32) {
        self.inner.note(output, scale, background);
    }

    fn present(&mut self, background: Color32) -> bool {
        self.with_surface(false, |inner| inner.present(background))
    }

    fn set_vsync(&mut self, vsync: bool) {
        self.configure(|inner| inner.set_vsync(vsync));
    }

    fn capture(&mut self) -> Option<Capture> {
        self.with_surface(None, |inner| inner.capture())
    }
}

#[cfg(test)]
mod tests;
