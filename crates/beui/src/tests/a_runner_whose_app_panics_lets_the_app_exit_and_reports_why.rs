use super::*;
use beui_core::app::SafeArea;
use beui_core::renderer::Loaded;
use beui_core::runner::{Launch, PlatformServices, RunOptions, Runner};

type Log = Signal<Vec<&'static str>>;

struct Nowhere;

impl PlatformServices for Nowhere {}

struct Failing {
    log: Log,
    frames: u32,
}

impl App for Failing {
    fn update(&mut self, _context: &Context, _rect: Rect) {
        self.frames += 1;
        self.log.update(|log| log.push("updated"));
        if self.frames == 2 {
            panic!("the second frame went wrong");
        }
    }

    fn exiting(&mut self) {
        self.log.update(|log| log.push("exiting"));
    }
}

#[test]
fn a_runner_whose_app_panics_lets_the_app_exit_and_reports_why() {
    let _root = crate::reactive::RootScope::new();
    let log = create_signal(Vec::new());
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Failing { log, frames: 0 }),
        layers: Vec::new(),
    });
    let loaded = Loaded {
        renderer: Box::new(crate::native::testing::Screen(vec2(200.0, 100.0))),
        fonts: None,
    };
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");
    assert!(runner.frame(&mut Nowhere, SafeArea::default()).is_some());
    assert!(runner.failure().is_none());

    assert!(
        runner.frame(&mut Nowhere, SafeArea::default()).is_none(),
        "a frame whose app panicked draws nothing"
    );
    assert!(runner.exited(), "the runner stops once its app panicked");
    assert_eq!(runner.failure(), Some("the second frame went wrong"));
    assert_eq!(
        log.update(std::mem::take),
        ["updated", "updated", "exiting"]
    );

    let finished = runner.finish().expect_err("the run ends in an error");
    assert!(
        finished.to_string().contains("the second frame went wrong"),
        "{finished}"
    );
    assert!(
        log.update(std::mem::take).is_empty(),
        "the app hears that it exits once"
    );
}
