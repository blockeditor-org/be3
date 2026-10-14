use super::*;
use beui_core::app::SafeArea;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, Platform, RunOptions, Runner};
use beui_core::window::WindowState;

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

struct Window(usize);

impl Platform for Window {
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
        options: RunOptions::new("runner"),
        context: context(),
        app: Box::new(Rejecting),
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
    let mut window = Window(0);

    runner.frame(&mut window, SafeArea::default());
    assert_eq!(std::mem::take(&mut window.0), 1);
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        std::mem::take(&mut window.0),
        0,
        "an unchanged window is not shown again"
    );

    runner.push(Event::Text("x".to_owned()));
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        std::mem::take(&mut window.0),
        1,
        "text the app turned away leaves the window as it was, but the platform's copy of the field moved, so it is shown again"
    );
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(std::mem::take(&mut window.0), 0);

    runner.push(Event::Ime(ImeEvent::FinishComposing));
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        std::mem::take(&mut window.0),
        1,
        "so is the end of a composition"
    );
}
