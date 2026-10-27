use std::error::Error;
use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::app::Setup;
use crate::color::Color32;
use crate::context::{Context, RendererInfo, WindowOutput};
use crate::font::FontBackend;
use crate::geometry::Vec2;

#[derive(Clone)]
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub trait WindowHandle: HasWindowHandle + HasDisplayHandle + Send + Sync {}

impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync> WindowHandle for T {}

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
