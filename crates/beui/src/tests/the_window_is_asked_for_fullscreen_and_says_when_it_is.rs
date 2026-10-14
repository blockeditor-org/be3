use super::*;
use beui_core::app::SafeArea;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, Platform, RunOptions, Runner};

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

struct Window(WriteSignal<Vec<String>>);

impl Platform for Window {
    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.0
            .update(|log| log.push(format!("fullscreen {fullscreen}")));
    }
}

struct Asking {
    asks: WriteSignal<Option<bool>>,
    seen: WriteSignal<bool>,
}

impl App for Asking {
    fn update(&mut self, context: &Context, _rect: Rect) {
        if let Some(fullscreen) = self.asks.update(Option::take) {
            context.set_fullscreen(fullscreen);
        }
        self.seen.set(context.fullscreen());
    }
}

#[test]
fn the_window_is_asked_for_fullscreen_and_says_when_it_is() {
    let _root = crate::reactive::RootScope::new();
    let (_, set_log) = create_signal(Vec::<String>::new());
    let (_, set_asks) = create_signal(None);
    let (seen, set_seen) = create_signal(false);
    let mut runner = Runner::new(Launch {
        options: RunOptions::new("runner"),
        context: context(),
        app: Box::new(Asking {
            asks: set_asks,
            seen: set_seen,
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
    let mut window = Window(set_log);
    let mut frame = |runner: &mut Runner| {
        runner.frame(&mut window, SafeArea::default());
    };

    set_asks.set(Some(true));
    frame(&mut runner);
    assert_eq!(set_log.update(std::mem::take), ["fullscreen true"]);
    assert!(!seen.get_untracked(), "asking is not being fullscreen");

    runner.reports().fullscreen(true);
    assert!(runner.has_events(), "the window's answer wakes the runner");
    frame(&mut runner);
    assert!(
        seen.get_untracked(),
        "the app hears that the window went fullscreen"
    );

    set_asks.set(Some(true));
    frame(&mut runner);
    assert!(
        set_log.update(std::mem::take).is_empty(),
        "a window already fullscreen is not asked again"
    );

    runner.reports().fullscreen(false);
    frame(&mut runner);
    assert!(
        !seen.get_untracked(),
        "the app hears that the window left fullscreen on its own"
    );
    set_asks.set(Some(true));
    frame(&mut runner);
    assert_eq!(
        set_log.update(std::mem::take),
        ["fullscreen true"],
        "the app can ask again after the window left"
    );
}
