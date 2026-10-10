mod input;
mod platform;
mod surface;

use std::cell::RefCell;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};
use std::time::Duration;

use beui_core::app::{App, SafeArea, Setup, Waker};
use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::Rect;
use beui_core::renderer::Loaded;
use beui_core::runner::{Adapter, Launch, RunOptions, Runner, Running};
use beui_core::screens::Screen;
use block_editor_plugin::{EditorHost, Frame, InputEvent, Region};
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::{PaintTarget, SurfaceRect};
use block_plugin_api::CursorIcon;

use input::Input;
use platform::HostPlatform;
use surface::{Surface, SurfaceRenderer};

pub struct PluginAdapter {
    host: EditorHost,
    surface: Rc<RefCell<Surface>>,
    launched: Rc<RefCell<Option<Runner>>>,
}

impl Adapter for PluginAdapter {
    fn name(&self) -> &'static str {
        "plugin"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move {
            let mut runner = Runner::new(launch);
            let waker = self.host.waker();
            let loaded = Loaded {
                renderer: Box::new(SurfaceRenderer::new(Rc::clone(&self.surface))),
                fonts: None,
            };
            runner.start(vec![loaded], Setup::new(Waker::new(move || waker.wake())))?;
            *self.launched.borrow_mut() = Some(runner);
            Ok(())
        })
    }
}

pub struct PluginSurface {
    runner: Runner,
    platform: HostPlatform,
    surface: Rc<RefCell<Surface>>,
    input: Input,
}

impl PluginSurface {
    pub fn launch(host: EditorHost, context: Context, app: impl App + 'static) -> Self {
        let surface = Rc::new(RefCell::new(Surface::default()));
        let launched = Rc::new(RefCell::new(None));
        let adapter = Box::new(PluginAdapter {
            host: host.clone(),
            surface: Rc::clone(&surface),
            launched: Rc::clone(&launched),
        });
        let launch = Launch {
            options: RunOptions::new("plugin"),
            context,
            app: Box::new(app),
        };
        let mut running = adapter.run(launch);
        match running
            .as_mut()
            .poll(&mut TaskContext::from_waker(TaskWaker::noop()))
        {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(error)) => panic!("a plugin surface could not start: {error}"),
            Poll::Pending => panic!("a plugin surface starts without waiting"),
        }
        let runner = launched
            .take()
            .expect("the plugin adapter hands its runner back once it has started");
        Self {
            runner,
            platform: HostPlatform::new(host),
            surface,
            input: Input::default(),
        }
    }

    pub fn context(&self) -> &Context {
        self.runner.context()
    }

    pub fn ratio(&self, region: &Region) -> f32 {
        let context = self.runner.context();
        context.set_pixels_per_point(region.scale_factor);
        region.scale_factor / context.pixels_per_point()
    }

    pub fn input(&mut self, region: &Region, event: &InputEvent) {
        let context = self.runner.context().clone();
        for event in self.input.translate(&context, region, event) {
            self.runner.push(event);
        }
    }

    pub fn update(&mut self, region: &Region) -> Frame {
        let [width, height] = region.pixels;
        let resized = {
            let mut surface = self.surface.borrow_mut();
            surface.age = region.age;
            surface.laid = region.rect;
            surface.screens = region
                .monitors
                .iter()
                .map(|monitor| {
                    Screen::new(&monitor.id, &monitor.name, monitor.rect)
                        .scaled(region.scale_factor)
                })
                .collect();
            let resized = surface.size != Some((width, height));
            surface.size = Some((width, height));
            resized
        };
        if resized && let Some(renderers) = self.runner.renderers() {
            renderers.resize(width, height);
        }
        if let Some(now) = block_editor_plugin::frame_time() {
            self.runner.context().set_clock(now);
        }
        self.platform.deliver_picks(self.runner.context());
        let ratio = self.ratio(region);
        let scale = region.scale_factor;
        let rect = region.rect;
        let area = SafeArea {
            left: rect.min.x * scale,
            top: rect.min.y * scale,
            right: width as f32 - rect.max.x * scale,
            bottom: height as f32 - rect.max.y * scale,
        };
        let Some(frame) = self.runner.frame(&mut self.platform, scale, area) else {
            return Frame::default();
        };
        let unscale = ratio.recip();
        let cursor = match self.runner.context().touch_emulation() {
            true => CursorIcon::Crosshair,
            false => self.platform.cursor,
        };
        Frame {
            changed: frame.pending,
            repaint_after: match frame.again() {
                true => Some(Duration::ZERO),
                false => (frame.repaint_after < Duration::MAX).then_some(frame.repaint_after),
            },
            cursor,
            content: None,
            painted: Vec::new(),
            floating: Vec::new(),
            claims: Vec::new(),
            ime: self
                .platform
                .ime
                .as_ref()
                .map(|area| block_editor_plugin::Ime {
                    rect: area.rect.scaled(unscale),
                    cursor: area.cursor.scaled(unscale),
                    text: area.text.as_ref().map(beui_plugin_input::protocol_ime_text),
                    keyboard: area.keyboard,
                }),
            handles_back: self.platform.handles_back,
            intercepted_keys: self.platform.intercepted_keys.clone(),
        }
    }

    pub fn to_region(&self, region: &Region, rect: Rect) -> Rect {
        let context = self.runner.context();
        rect.scaled(context.pixels_per_point() / region.scale_factor)
    }

    pub fn pointer_locked(&self) -> bool {
        self.platform.locked
    }

    pub fn take_output(&mut self) -> Option<FrameOutput> {
        self.runner.take_output()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn paint(&mut self, target: &PaintTarget<'_>) -> Vec<SurfaceRect> {
        self.surface.borrow_mut().aim(target);
        self.runner.present();
        self.surface.borrow_mut().take_drawn()
    }
}

#[cfg(test)]
mod tests;
