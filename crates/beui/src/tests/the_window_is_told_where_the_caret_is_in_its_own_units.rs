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
        Some(vec2(300.0, 150.0))
    }

    fn prepare(&mut self, output: &FrameOutput, _scale: f32, _background: Color32) -> bool {
        output.changed
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

struct Window(Vec<Option<Rect>>);

impl Platform for Window {
    fn show_window(&mut self, window: &WindowState, _shown: &WindowState) {
        self.0.push(window.ime.as_ref().map(|area| area.cursor));
    }
}

struct Field;

impl App for Field {
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
fn the_window_is_told_where_the_caret_is_in_its_own_units() {
    let mut runner = Runner::new(Launch {
        options: RunOptions::new("runner"),
        context: context(),
        app: Box::new(Field),
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
    runner.reports().scaled(1.5);
    runner.context().set_zoom_factor(2.0);
    let mut window = Window(Vec::new());

    runner.frame(&mut window, SafeArea::default());
    assert_eq!(runner.context().pixels_per_point(), 3.0);
    assert_eq!(
        std::mem::take(&mut window.0),
        [Some(Rect::from_min_max(pos2(24.0, 20.0), pos2(26.0, 40.0)))],
        "the caret is in the window's units, twice its points at a zoom of 2"
    );

    runner.frame(&mut window, SafeArea::default());
    assert!(
        std::mem::take(&mut window.0).is_empty(),
        "an unchanged window is not told again"
    );

    runner.context().set_zoom_factor(1.0);
    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        std::mem::take(&mut window.0),
        [Some(Rect::from_min_max(pos2(12.0, 10.0), pos2(13.0, 20.0)))],
        "zooming moves the caret in the window although the app's points stayed put"
    );
}
