use super::*;
use beui::{native::Simulation, native::WindowSize};
use beui_core::app::SafeArea;
use beui_core::renderer::Loaded;
use beui_core::runner::{Launch, RunOptions, Runner};

struct Asking {
    asked: bool,
    seen: Signal<bool>,
}

impl App for Asking {
    fn update(&mut self, context: &Context, _rect: Rect) {
        if !std::mem::replace(&mut self.asked, true) {
            context.set_fullscreen(true);
        }
        self.seen.set(context.fullscreen());
    }
}

#[test]
fn a_simulated_window_tells_the_app_it_went_fullscreen_in_the_next_frame() {
    let _root = crate::reactive::RootScope::new();
    let seen = create_signal(false);
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Asking { asked: false, seen }),
        layers: Vec::new(),
    });
    let mut window = Simulation::new(
        WindowSize {
            size: vec2(200.0, 100.0),
            scale: 1.0,
        },
        Some(vec2(400.0, 300.0)),
        runner.reports().clone(),
    );
    runner
        .start(
            Loaded {
                renderer: Box::new(crate::native::testing::Screen(vec2(200.0, 100.0))),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");

    let frame = runner
        .frame(&mut window, SafeArea::default())
        .expect("a loaded runner draws");
    assert!(!seen.get_untracked(), "the app has only asked");
    assert!(
        frame.again(),
        "the simulated window's answer asks for the frame that delivers it"
    );

    runner.frame(&mut window, SafeArea::default());
    assert!(
        seen.get_untracked(),
        "the app hears that the window went fullscreen"
    );
}
