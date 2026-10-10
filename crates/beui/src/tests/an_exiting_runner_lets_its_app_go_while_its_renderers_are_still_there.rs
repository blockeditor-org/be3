use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use beui_core::app::SafeArea;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, Platform, RunOptions, Runner};

type Log = Rc<RefCell<Vec<&'static str>>>;

struct Screen(Log);

impl Renderer for Screen {
    fn name(&self) -> &'static str {
        "screen"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(200.0, 100.0))
    }

    fn prepare(&mut self, output: &FrameOutput, _scale: f32, _background: Color32) -> bool {
        output.changed
    }

    fn present(&mut self, _background: Color32) -> bool {
        self.0.borrow_mut().push("presented");
        false
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.0.borrow_mut().push("renderer dropped");
    }
}

struct Nowhere;

impl Platform for Nowhere {
    fn copy(&mut self, _text: String) {}

    fn paste(&mut self) -> Option<String> {
        None
    }

    fn pick_file(&mut self, _request: FilePickRequest) {}
}

struct Holding(Log);

impl App for Holding {
    fn update(&mut self, _context: &Context, _rect: Rect) {
        self.0.borrow_mut().push("updated");
    }

    fn exiting(&mut self) {
        self.0.borrow_mut().push("exiting");
    }
}

impl Drop for Holding {
    fn drop(&mut self) {
        self.0.borrow_mut().push("app dropped");
    }
}

#[test]
fn an_exiting_runner_lets_its_app_go_while_its_renderers_are_still_there() {
    let log = Log::default();
    let mut runner = Runner::new(Launch {
        options: RunOptions::new("runner"),
        context: context(),
        app: Box::new(Holding(log.clone())),
    });
    let loaded = vec![Loaded {
        renderer: Box::new(Screen(log.clone())),
        fonts: None,
    }];
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");
    runner
        .frame(&mut Nowhere, 1.0, SafeArea::default())
        .expect("a loaded runner draws");
    runner.present();

    runner.exit();
    assert_eq!(
        log.take(),
        ["updated", "presented", "exiting", "app dropped"]
    );
    runner.present();
    assert!(
        log.take().is_empty(),
        "an exited runner presents nothing more"
    );
    assert!(
        runner
            .frame(&mut Nowhere, 1.0, SafeArea::default())
            .is_none(),
        "an exited runner draws nothing more"
    );
    runner.exit();
    assert!(log.take().is_empty(), "the app hears that it exits once");

    drop(runner);
    assert_eq!(log.take(), ["renderer dropped"]);
}
