use super::*;
use beui_core::app::SafeArea;
use beui_core::app::automation::{Simulation, WindowSize};
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, RunOptions, Runner};

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

struct Asking {
    asked: bool,
    seen: Rc<Cell<bool>>,
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
    let seen = Rc::new(Cell::new(false));
    let mut runner = Runner::new(Launch {
        options: RunOptions::new("runner"),
        context: context(),
        app: Box::new(Asking {
            asked: false,
            seen: seen.clone(),
        }),
    });
    let mut window = Simulation::new(
        WindowSize {
            size: vec2(200.0, 100.0),
            scale: 1.0,
        },
        Some(vec2(400.0, 300.0)),
        runner.reports(),
    );
    runner
        .start(
            vec![Loaded {
                renderer: Box::new(Screen),
                fonts: None,
            }],
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");

    let frame = runner
        .frame(&mut window, 1.0, SafeArea::default())
        .expect("a loaded runner draws");
    assert!(!seen.get(), "the app has only asked");
    assert!(
        frame.again(),
        "the simulated window's answer asks for the frame that delivers it"
    );

    runner.frame(&mut window, 1.0, SafeArea::default());
    assert!(seen.get(), "the app hears that the window went fullscreen");
}
