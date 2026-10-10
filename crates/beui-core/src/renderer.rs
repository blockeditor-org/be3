use std::error::Error;
use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::app::Setup;
use crate::app::automation::Capture;
use crate::color::Color32;
use crate::context::{Context, FrameOutput, RendererChoices, RendererInfo};
use crate::font::FontBackend;
use crate::geometry::Vec2;
use crate::screens::Screen;

pub trait WindowHandle: HasWindowHandle + HasDisplayHandle + Send + Sync {}

impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync> WindowHandle for T {}

pub trait Renderer {
    fn name(&self) -> &'static str;

    fn info(&self) -> RendererInfo;

    fn provide(&self, _setup: &mut Setup) {}

    fn set_active(&mut self, _active: bool) {}

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

    fn screens(&self) -> Option<Vec<Screen>> {
        None
    }

    fn prepare(&mut self, output: &FrameOutput, scale: f32, background: Color32) -> bool;

    fn present(&mut self, background: Color32) -> bool;

    fn capture(&mut self) -> Option<Capture> {
        None
    }
}

pub struct Loaded {
    pub renderer: Box<dyn Renderer>,
    pub fonts: Option<Box<dyn FontBackend>>,
}

pub fn any_loaded(
    results: impl IntoIterator<Item = Result<Loaded, Box<dyn Error>>>,
    mut warn: impl FnMut(&dyn Error),
) -> Result<Vec<Loaded>, Box<dyn Error>> {
    let mut loaded = Vec::new();
    let mut failure = None;
    for result in results {
        match result {
            Ok(renderer) => loaded.push(renderer),
            Err(error) => {
                warn(&*error);
                failure.get_or_insert(error);
            }
        }
    }
    match (loaded.is_empty(), failure) {
        (true, Some(error)) => Err(error),
        _ => Ok(loaded),
    }
}

struct Slot {
    renderer: Box<dyn Renderer>,
    own_fonts: bool,
    fonts: Option<Box<dyn FontBackend>>,
}

pub struct Renderers {
    slots: Vec<Slot>,
    active: usize,
    shared_fonts: Option<Box<dyn FontBackend>>,
    window: Option<Arc<dyn WindowHandle>>,
    size: Option<(u32, u32)>,
}

impl Renderers {
    pub fn new(context: &Context, loaded: Vec<Loaded>) -> Result<Self, Box<dyn Error>> {
        if loaded.is_empty() {
            return Err("no renderer was loaded".into());
        }
        let slots = loaded
            .into_iter()
            .map(|Loaded { renderer, fonts }| Slot {
                renderer,
                own_fonts: fonts.is_some(),
                fonts,
            })
            .collect();
        let mut renderers = Self {
            slots,
            active: 0,
            shared_fonts: None,
            window: None,
            size: None,
        };
        for slot in &mut renderers.slots[1..] {
            slot.renderer.set_active(false);
        }
        if let Some(fonts) = renderers.slots[0].fonts.take() {
            renderers.shared_fonts = Some(context.replace_fonts(fonts));
        }
        renderers.slots[0].renderer.set_active(true);
        renderers.publish(context);
        Ok(renderers)
    }

    pub fn provide(&self, setup: &mut Setup) {
        for slot in &self.slots {
            slot.renderer.provide(setup);
        }
    }

    pub fn recover(&mut self) -> Result<bool, Box<dyn Error>> {
        let mut replaced = false;
        for slot in &mut self.slots {
            replaced |= slot.renderer.recover()?;
        }
        Ok(replaced)
    }

    pub fn attach(
        &mut self,
        window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn Error>> {
        self.window = Some(window.clone());
        self.size = Some((width, height));
        self.active_mut().attach(window, width, height)
    }

    pub fn attached(&self) -> bool {
        self.window.is_some()
    }

    pub fn detach(&mut self) {
        self.window = None;
        self.active_mut().detach();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.size = Some((width, height));
        self.active_mut().resize(width, height);
    }

    pub fn physical(&self) -> Option<Vec2> {
        self.slots[self.active].renderer.physical()
    }

    pub fn screens(&self) -> Option<Vec<Screen>> {
        self.slots[self.active].renderer.screens()
    }

    pub fn prepare(&mut self, output: &FrameOutput, scale: f32, background: Color32) -> bool {
        self.active_mut().prepare(output, scale, background)
    }

    pub fn present(&mut self, background: Color32) -> bool {
        self.active_mut().present(background)
    }

    pub fn capture(&mut self) -> Option<Capture> {
        self.active_mut().capture()
    }

    pub fn follow_choice(&mut self, context: &Context) -> Result<bool, Box<dyn Error>> {
        match context.take_renderer_choice() {
            Some(index) => self.choose(context, index),
            None => Ok(false),
        }
    }

    pub fn choose(&mut self, context: &Context, index: usize) -> Result<bool, Box<dyn Error>> {
        if index == self.active || index >= self.slots.len() {
            return Ok(false);
        }
        let previous = self.active;
        self.hide(previous);
        if let Err(error) = self.show(index) {
            self.hide(index);
            self.show(previous)?;
            return Err(error);
        }
        self.swap_fonts(context, previous, index);
        self.active = index;
        self.publish(context);
        Ok(true)
    }

    fn hide(&mut self, index: usize) {
        let renderer = &mut self.slots[index].renderer;
        renderer.detach();
        renderer.set_active(false);
    }

    fn show(&mut self, index: usize) -> Result<(), Box<dyn Error>> {
        let renderer = &mut self.slots[index].renderer;
        renderer.set_active(true);
        match (&self.window, self.size) {
            (Some(window), Some((width, height))) => renderer.attach(window.clone(), width, height),
            (None, Some((width, height))) => {
                renderer.resize(width, height);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn swap_fonts(&mut self, context: &Context, previous: usize, next: usize) {
        if !(self.slots[previous].own_fonts || self.slots[next].own_fonts) {
            return;
        }
        let incoming = match self.slots[next].own_fonts {
            true => self.slots[next].fonts.take(),
            false => self.shared_fonts.take(),
        };
        let Some(incoming) = incoming else {
            return;
        };
        let outgoing = context.replace_fonts(incoming);
        match self.slots[previous].own_fonts {
            true => self.slots[previous].fonts = Some(outgoing),
            false => self.shared_fonts = Some(outgoing),
        }
    }

    fn publish(&self, context: &Context) {
        let renderer = &self.slots[self.active].renderer;
        context.set_renderer_info(renderer.info());
        context.set_renderers(RendererChoices {
            names: self.slots.iter().map(|slot| slot.renderer.name()).collect(),
            active: self.active,
        });
    }

    fn active_mut(&mut self) -> &mut dyn Renderer {
        &mut *self.slots[self.active].renderer
    }
}
