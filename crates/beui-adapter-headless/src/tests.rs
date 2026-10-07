use super::*;

mod a_wake_from_another_thread_runs_another_frame;
mod an_app_that_closes_its_window_exits_before_it_is_dropped;

use std::cell::RefCell;
use std::rc::Rc;

use beui_core::color::Color32;
use beui_core::context::{Context, FrameOutput, RendererInfo};
use beui_core::geometry::{Rect, Vec2};
use beui_core::runner::RunOptions;

struct Blank(Vec2);

impl beui_core::renderer::Renderer for Blank {
    fn name(&self) -> &'static str {
        "blank"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.0 = Vec2::new(width as f32, height as f32);
    }

    fn physical(&self) -> Option<Vec2> {
        Some(self.0)
    }

    fn prepare(&mut self, _output: &FrameOutput, _scale: f32, _background: Color32) -> bool {
        true
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

pub type Log = Rc<RefCell<Vec<String>>>;

pub fn run_headless(app: impl beui_core::app::App + 'static) -> Result<(), Box<dyn Error>> {
    let adapter = Box::new(Headless::new(|width, height| {
        Ok(vec![Loaded {
            renderer: Box::new(Blank(Vec2::new(width as f32, height as f32))),
            fonts: None,
        }])
    }));
    let launch = Launch {
        options: RunOptions::new("headless test"),
        context: Context::new(beui_font_freetype::FreetypeFonts::default()),
        app: Box::new(app),
    };
    pollster::block_on(adapter.run(launch))
}

pub struct Recorder<F: FnMut(&Context, usize)> {
    pub log: Log,
    pub frames: usize,
    pub on_frame: F,
}

impl<F: FnMut(&Context, usize)> beui_core::app::App for Recorder<F> {
    fn update(&mut self, context: &Context, _rect: Rect) {
        self.frames += 1;
        self.log.borrow_mut().push(format!("frame {}", self.frames));
        (self.on_frame)(context, self.frames);
    }

    fn setup(&mut self, setup: &Setup) {
        let waker = setup.waker.clone();
        self.log.borrow_mut().push("setup".to_owned());
        std::thread::spawn(move || waker.wake());
    }

    fn exiting(&mut self) {
        self.log.borrow_mut().push("exiting".to_owned());
    }
}

impl<F: FnMut(&Context, usize)> Drop for Recorder<F> {
    fn drop(&mut self) {
        self.log.borrow_mut().push("dropped".to_owned());
    }
}
