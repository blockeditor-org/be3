use super::*;
use beui_core::program::SafeArea;
use beui_runner::renderer::Loaded;
use beui_runner::{Launch, PlatformServices, RunOptions, Runner};

struct Window(Signal<usize>);

impl PlatformServices for Window {
    fn raise_window(&mut self) {
        self.0.update(|raised| *raised += 1);
    }
}

struct Asking(Signal<bool>);

impl Program for Asking {
    fn update(&mut self, context: &Context, _rect: Rect) {
        if self.0.update(std::mem::take) {
            context.raise_window();
        }
    }
}

#[test]
fn the_window_is_raised_once_when_the_program_asks() {
    let _root = crate::reactive::RootScope::new();
    let raised = create_signal(0);
    let asks = create_signal(false);
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        program: Box::new(Asking(asks)),
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
    let mut window = Window(raised);
    let mut frame = |runner: &mut Runner| {
        runner.frame(&mut window, SafeArea::default());
    };

    frame(&mut runner);
    assert_eq!(raised.get_untracked(), 0, "a window is not raised unasked");
    asks.set(true);
    frame(&mut runner);
    assert_eq!(raised.get_untracked(), 1, "asking raises the window");
    frame(&mut runner);
    assert_eq!(raised.get_untracked(), 1, "one ask raises the window once");
}
