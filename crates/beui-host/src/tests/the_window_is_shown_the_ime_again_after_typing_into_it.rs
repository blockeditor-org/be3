use super::*;
use beui_core::app::SafeArea;
use beui_core::window::WindowState;
use beui_runner::renderer::Loaded;
use beui_runner::{Launch, PlatformServices, RunOptions, Runner};

struct Window(usize);

impl PlatformServices for Window {
    fn show_window(&mut self, _window: &WindowState, _shown: &WindowState) {
        self.0 += 1;
    }
}

struct Rejecting;

impl App for Rejecting {
    fn update(&mut self, context: &Context, _rect: Rect) {
        context.set_ime_area(Some(ImeArea {
            rect: Rect::from_min_max(pos2(10.0, 10.0), pos2(30.0, 20.0)),
            cursor: Rect::from_min_max(pos2(12.0, 10.0), pos2(13.0, 20.0)),
            text: None,
            keyboard: Keyboard::Shown { asks: 0 },
        }));
    }
}

#[test]
fn the_window_is_shown_the_ime_again_after_typing_into_it() {
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Rejecting),
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
    let mut window = Window(0);

    runner.frame(&mut window, SafeArea::default());
    assert_eq!(std::mem::take(&mut window.0), 1);
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        std::mem::take(&mut window.0),
        0,
        "an unchanged window is not shown again"
    );

    runner.push(Input::Text("x".to_owned()));
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        std::mem::take(&mut window.0),
        1,
        "text the app turned away leaves the window as it was, but the platform's copy of the field moved, so it is shown again"
    );
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(std::mem::take(&mut window.0), 0);

    runner.push(Input::Ime(ImeEvent::FinishComposing));
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        std::mem::take(&mut window.0),
        1,
        "so is the end of a composition"
    );
}
