use super::*;
use beui_core::app::SafeArea;
use beui_core::damage::Region;
use beui_runner::renderer::{Loaded, Renderer};
use beui_runner::{Incoming, Launch, Layer, LayerFrame, RunOptions, Runner, Shown};
use beui_core::window::{WindowHosting, WindowId, WindowReport, WindowState};

struct Screen(Rc<RefCell<Vec<Color32>>>);

impl Renderer for Screen {
    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(400.0, 300.0))
    }

    fn prepare(
        &mut self,
        _output: &crate::native::WindowOutput,
        _scale: f32,
        background: Color32,
    ) -> bool {
        self.0.borrow_mut().push(background);
        false
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

struct Sized(Rc<RefCell<Vec<Rect>>>);

impl App for Sized {
    fn update(&mut self, _context: &Context, rect: Rect) {
        self.0.borrow_mut().push(rect);
    }
}

struct Host {
    reported: bool,
    seen: Rc<RefCell<Vec<(Vec2, Vec2)>>>,
}

impl Layer for Host {
    fn attach(&mut self, context: &Context) {
        context.set_window_hosting(WindowHosting::Any);
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, _frame: &mut LayerFrame<'_>) {
        if !std::mem::replace(&mut self.reported, true) {
            incoming.push(Incoming::Window(WindowReport::Resized {
                window: WindowId::MAIN,
                size: vec2(120.0, 80.0),
            }));
        }
    }

    fn output(&mut self, shown: &mut Shown, _output: &FrameOutput, frame: &mut LayerFrame<'_>) {
        let view = frame.view;
        let main = shown.main().expect("the app shows its main window");
        self.seen.borrow_mut().push((main.size, view.platform));
        let mut window = WindowState::default();
        window.appearance.clear_color = Color32::WHITE;
        shown.windows = vec![crate::native::WindowOutput {
            id: WindowId::MAIN,
            size: view.platform / view.pixels_per_point,
            layers: Rc::new(Vec::new()),
            window,
            changed: true,
            damage: Region::NOTHING,
            moved: None,
            pixels_per_point: view.pixels_per_point,
        }];
    }
}

struct Nowhere;

impl beui_runner::PlatformServices for Nowhere {}

#[test]
fn a_window_hosting_layer_sizes_the_main_window_and_shows_what_it_composes() {
    let rects = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let backgrounds = Rc::new(RefCell::new(Vec::new()));
    let context = context();
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context.clone(),
        app: Box::new(Sized(Rc::clone(&rects))),
        layers: vec![Box::new(Host {
            reported: false,
            seen: Rc::clone(&seen),
        })],
    });
    assert!(
        context.opens_windows(),
        "a window hosting layer opens the app's windows"
    );
    runner
        .start(
            Loaded {
                renderer: Box::new(Screen(Rc::clone(&backgrounds))),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");

    runner.frame(&mut Nowhere, SafeArea::default());

    assert_eq!(
        rects.take(),
        [Rect::from_min_size(Pos2::ZERO, vec2(120.0, 80.0))],
        "the app lays out at the size the host gave its main window, not the platform's"
    );
    assert_eq!(
        seen.take(),
        [(vec2(120.0, 80.0), vec2(400.0, 300.0))],
        "the host sees the app's main window and the platform window it shows it in"
    );
    assert_eq!(
        backgrounds.take(),
        [Color32::WHITE],
        "the renderers draw what the host composed"
    );
}
