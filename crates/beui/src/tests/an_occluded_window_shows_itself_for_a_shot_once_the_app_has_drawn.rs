use super::*;
use beui::drive::{Drive, Reply, ask};
use beui::{Simulation, WindowSize};
use beui_core::app::SafeArea;
use beui_core::renderer::Capture;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, RunOptions, Runner};
use std::cell::Cell;
use std::future::Future;
use std::rc::Rc;
use std::task::Poll;

struct Screen {
    prepared: Rc<Cell<u32>>,
}

impl Renderer for Screen {
    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(20.0, 10.0))
    }

    fn prepare(
        &mut self,
        _output: &crate::WindowOutput,
        _scale: f32,
        _background: Color32,
    ) -> bool {
        self.prepared.set(self.prepared.get() + 1);
        true
    }

    fn note(&mut self, _output: &crate::WindowOutput, _scale: f32, _background: Color32) {}

    fn present(&mut self, _background: Color32) -> bool {
        false
    }

    fn capture(&mut self) -> Option<Capture> {
        Some(Capture {
            width: 20,
            height: 10,
            rgba: vec![0; 20 * 10 * 4],
        })
    }
}

struct Showing {
    document: Document,
    seen: Rc<Cell<Vec<bool>>>,
    drawing: Rc<Cell<u32>>,
}

impl App for Showing {
    fn update(&mut self, context: &Context, rect: Rect) {
        let mut seen = self.seen.take();
        seen.push(context.seen());
        self.seen.set(seen);
        if context.seen() {
            self.drawing.set(self.drawing.get().saturating_sub(1));
        }
        self.document.show(context, rect);
    }

    fn drawing(&self) -> bool {
        self.drawing.get() > 0
    }
}

#[test]
fn an_occluded_window_shows_itself_for_a_shot_once_the_app_has_drawn() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(|| {
        view! {
            <Text color=Color32::WHITE>"Shown"</Text>
        }
    });
    let prepared = Rc::new(Cell::new(0));
    let seen = Rc::new(Cell::new(Vec::new()));
    let drawing = Rc::new(Cell::new(3));
    let inbox = beui::drive::Inbox::default();
    let options = RunOptions::default();
    let mut runner = Runner::new(Launch {
        options,
        context: context(),
        app: Box::new(Showing {
            document,
            seen: Rc::clone(&seen),
            drawing: Rc::clone(&drawing),
        }),
        layers: vec![Box::new(Drive::new(inbox.clone()))],
    });
    runner.set_occluded(true);
    let mut window = Simulation::new(
        WindowSize {
            size: vec2(20.0, 10.0),
            scale: 1.0,
        },
        None,
        runner.reports().clone(),
    );
    runner
        .start(
            Loaded {
                renderer: Box::new(Screen {
                    prepared: Rc::clone(&prepared),
                }),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");

    runner.frame(&mut window, SafeArea::default());
    runner.present();
    assert_eq!(seen.take(), [false], "nobody looks at an occluded window");
    assert_eq!(prepared.get(), 0, "an unseen frame is not drawn");

    let mut answered = std::pin::pin!(ask(&inbox, vec!["shot".to_owned()]));
    let mut waiting = std::task::Context::from_waker(std::task::Waker::noop());
    let mut frames = 0;
    let reply = loop {
        frames += 1;
        assert!(frames < 10, "the shot was never taken");
        runner.frame(&mut window, SafeArea::default());
        runner.present();
        if let Poll::Ready(reply) = answered.as_mut().poll(&mut waiting) {
            break reply;
        }
    };

    assert!(matches!(reply, Reply::Image { .. }), "the shot is an image");
    assert_eq!(
        seen.take(),
        [true, true, true],
        "the app is seen from the frame that takes the shot until it has drawn"
    );
    assert_eq!(drawing.get(), 0, "the shot waited for the app to draw");
    assert_eq!(
        prepared.get(),
        3,
        "each frame the shot is looked at is drawn"
    );

    runner.frame(&mut window, SafeArea::default());
    assert_eq!(
        seen.take(),
        [false],
        "the window is unseen once the shot is taken"
    );
}
