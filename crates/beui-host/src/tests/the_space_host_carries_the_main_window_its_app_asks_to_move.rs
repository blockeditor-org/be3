use super::*;
use beui::window::{WindowId, WindowRequest};
use beui_core::app::SafeArea;
use beui_runner::renderer::{Loaded, Renderer};
use beui_runner::{Launch, RunOptions, Runner};
use std::cell::Cell;

struct Screen;

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
        _background: Color32,
    ) -> bool {
        false
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

struct Mover(Rc<Cell<bool>>);

impl App for Mover {
    fn update(&mut self, context: &Context, _rect: Rect) {
        if self.0.take() {
            context.request_window(WindowRequest::BeginMove(WindowId::MAIN));
        }
    }
}

struct Nowhere;

impl beui_runner::PlatformServices for Nowhere {}

#[test]
fn the_space_host_carries_the_main_window_its_app_asks_to_move() {
    let moving = Rc::new(Cell::new(false));
    let context = context();
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context.clone(),
        app: Box::new(Mover(Rc::clone(&moving))),
        layers: vec![Box::new(beui_layer_space::Space::new())],
    });
    runner
        .start(
            Loaded {
                renderer: Box::new(Screen),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");
    runner.frame(&mut Nowhere, SafeArea::default());
    assert_eq!(
        context.window_size(WindowId::MAIN),
        Some(vec2(400.0, 300.0)),
        "space gives the main window the platform window's size"
    );
    assert_eq!(context.window_placement(WindowId::MAIN), None);

    let middle = pos2(200.0, 150.0);
    for pressed in [true, false] {
        moving.set(pressed);
        runner.push(Input::PointerButton {
            pos: middle,
            button: PointerButton::Primary,
            pressed,
        });
        runner.frame(&mut Nowhere, SafeArea::default());
    }

    let placement = context
        .window_placement(WindowId::MAIN)
        .expect("the carry ends with the window placed");
    assert!(
        placement.0.starts_with("space "),
        "the placement is the host's own: {placement:?}"
    );
}
