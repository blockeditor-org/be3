use super::*;
use crate::native::{Simulation, WindowPictures, WindowSize};
use beui_core::program::SafeArea;
use beui_runner::renderer::Capture;
use beui_runner::renderer::{Loaded, Renderer};
use beui_runner::{Launch, RunOptions, Runner};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Screen {
    presented: Rc<Cell<u32>>,
}

impl Renderer for Screen {
    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(2.0, 1.0))
    }

    fn prepare(
        &mut self,
        _output: &crate::native::WindowOutput,
        _scale: f32,
        _background: Color32,
    ) -> bool {
        true
    }

    fn present(&mut self, _background: Color32) -> bool {
        self.presented.set(self.presented.get() + 1);
        false
    }

    fn capture(&mut self) -> Option<Capture> {
        let shade = u8::try_from(self.presented.get()).unwrap_or(u8::MAX);
        Some(Capture {
            width: 2,
            height: 1,
            rgba: vec![shade; 2 * 4],
        })
    }
}

struct Asking {
    pictures: Rc<RefCell<Option<WindowPictures>>>,
}

impl Program for Asking {
    fn update(&mut self, _context: &Context, _rect: Rect) {}

    fn setup(&mut self, setup: &Setup) {
        *self.pictures.borrow_mut() = setup.get::<WindowPictures>().cloned();
    }
}

#[test]
fn a_window_picture_is_the_frame_last_presented() {
    let pictures = Rc::new(RefCell::new(None));
    let presented = Rc::new(Cell::new(0));
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        program: Box::new(Asking {
            pictures: Rc::clone(&pictures),
        }),
        layers: Vec::new(),
    });
    let mut window = Simulation::new(
        WindowSize {
            size: vec2(2.0, 1.0),
            scale: 1.0,
        },
        None,
        runner.reports().clone(),
    );
    runner
        .start(
            Loaded {
                renderer: Box::new(Screen {
                    presented: Rc::clone(&presented),
                }),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");
    runner.frame(&mut window, SafeArea::default());
    runner.present();

    let pictures = pictures
        .borrow()
        .clone()
        .expect("the runner provides the window's pictures");
    let answered = Rc::new(RefCell::new(Vec::new()));
    pictures.take({
        let answered = Rc::clone(&answered);
        move |picture| answered.borrow_mut().push(picture)
    });
    assert!(answered.borrow().is_empty(), "the answer waits for a frame");

    runner.frame(&mut window, SafeArea::default());
    runner.present();

    let answered = answered.take();
    assert_eq!(answered.len(), 1, "the asker is answered once");
    let picture = answered[0]
        .as_ref()
        .expect("a presented window has a picture");
    assert_eq!((picture.width(), picture.height()), (2, 1));
    assert_eq!(
        picture.pixels()[0],
        u8::try_from(presented.get()).unwrap(),
        "the picture is of the latest presented frame"
    );
}
