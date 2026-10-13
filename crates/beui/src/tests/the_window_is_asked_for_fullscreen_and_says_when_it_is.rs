use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use beui_core::app::SafeArea;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, Platform, RunOptions, Runner};

type Log = Rc<RefCell<Vec<String>>>;

struct Screen;

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
        false
    }
}

struct Window(Log);

impl Platform for Window {
    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.0.borrow_mut().push(format!("fullscreen {fullscreen}"));
    }
}

struct Asking {
    asks: Rc<Cell<Option<bool>>>,
    seen: Rc<Cell<bool>>,
}

impl App for Asking {
    fn update(&mut self, context: &Context, _rect: Rect) {
        if let Some(fullscreen) = self.asks.take() {
            context.set_fullscreen(fullscreen);
        }
        self.seen.set(context.fullscreen());
    }
}

#[test]
fn the_window_is_asked_for_fullscreen_and_says_when_it_is() {
    let log = Log::default();
    let asks = Rc::new(Cell::new(None));
    let seen = Rc::new(Cell::new(false));
    let mut runner = Runner::new(Launch {
        options: RunOptions::new("runner"),
        context: context(),
        app: Box::new(Asking {
            asks: asks.clone(),
            seen: seen.clone(),
        }),
    });
    runner
        .start(
            vec![Loaded {
                renderer: Box::new(Screen),
                fonts: None,
            }],
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");
    let mut window = Window(log.clone());
    let mut frame = |runner: &mut Runner| {
        runner.frame(&mut window, 1.0, SafeArea::default());
    };

    asks.set(Some(true));
    frame(&mut runner);
    assert_eq!(log.take(), ["fullscreen true"]);
    assert!(!seen.get(), "asking is not being fullscreen");

    runner.fullscreen_changed(true);
    assert!(runner.has_events(), "the window's answer wakes the runner");
    frame(&mut runner);
    assert!(seen.get(), "the app hears that the window went fullscreen");

    asks.set(Some(true));
    frame(&mut runner);
    assert!(
        log.take().is_empty(),
        "a window already fullscreen is not asked again"
    );

    runner.fullscreen_changed(false);
    frame(&mut runner);
    assert!(
        !seen.get(),
        "the app hears that the window left fullscreen on its own"
    );
    asks.set(Some(true));
    frame(&mut runner);
    assert_eq!(
        log.take(),
        ["fullscreen true"],
        "the app can ask again after the window left"
    );
}
