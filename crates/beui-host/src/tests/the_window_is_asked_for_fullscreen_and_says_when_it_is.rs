use super::*;
use beui_core::app::SafeArea;
use beui_runner::renderer::Loaded;
use beui_runner::{Launch, PlatformServices, RunOptions, Runner};

struct Window(Signal<Vec<String>>);

impl PlatformServices for Window {
    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.0
            .update(|log| log.push(format!("fullscreen {fullscreen}")));
    }
}

struct Asking {
    asks: Signal<Option<bool>>,
    seen: Signal<bool>,
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
    let log = create_signal(Vec::<String>::new());
    let asks = create_signal(None);
    let seen = create_signal(false);
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Asking { asks, seen }),
        layers: Vec::new(),
    });
    runner
        .start(
            Loaded {
                renderer: Box::new(crate::native::testing::Screen(vec2(200.0, 100.0))),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");
    let mut window = Window(log);
    let mut frame = |runner: &mut Runner| {
        runner.frame(&mut window, SafeArea::default());
    };

    asks.set(Some(true));
    frame(&mut runner);
    assert_eq!(log.update(std::mem::take), ["fullscreen true"]);
    assert!(!seen.get_untracked(), "asking is not being fullscreen");

    runner.reports().fullscreen(true);
    assert!(runner.has_events(), "the window's answer wakes the runner");
    frame(&mut runner);
    assert!(
        seen.get_untracked(),
        "the app hears that the window went fullscreen"
    );

    asks.set(Some(true));
    frame(&mut runner);
    assert!(
        log.update(std::mem::take).is_empty(),
        "a window already fullscreen is not asked again"
    );

    runner.reports().fullscreen(false);
    frame(&mut runner);
    assert!(
        !seen.get_untracked(),
        "the app hears that the window left fullscreen on its own"
    );
    asks.set(Some(true));
    frame(&mut runner);
    assert_eq!(
        log.update(std::mem::take),
        ["fullscreen true"],
        "the app can ask again after the window left"
    );
}
