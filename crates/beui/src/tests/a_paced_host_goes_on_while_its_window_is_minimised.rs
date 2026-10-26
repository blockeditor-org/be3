use super::*;
use beui_core::app::SafeArea;
use beui_core::context::FrameOutput;
use beui_core::renderer::Loaded;
use beui_core::runner::{
    HostView, Hosted, Incoming, Launch, Platform, RunOptions, Runner, WindowHost,
};
use std::time::Instant;

struct Nowhere;

impl Platform for Nowhere {}

struct Paced {
    paces: bool,
    framed: Rc<RefCell<Vec<Vec2>>>,
}

impl WindowHost for Paced {
    fn pace(&mut self) -> Option<Instant> {
        self.paces.then(Instant::now)
    }

    fn input(&mut self, _incoming: &mut Vec<Incoming>, _view: &HostView) {}

    fn frame(&mut self, output: &FrameOutput, view: &HostView) -> Hosted {
        self.framed.borrow_mut().push(view.platform);
        Hosted {
            window: output.main().clone(),
            repaint_after: Duration::ZERO,
        }
    }
}

struct Still;

impl App for Still {
    fn update(&mut self, _context: &Context, _rect: Rect) {}
}

fn frames_while_minimised(paces: bool) -> Vec<Vec2> {
    let framed = Rc::new(RefCell::new(Vec::new()));
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Still),
        host: Some(Box::new(Paced {
            paces,
            framed: framed.clone(),
        })),
    });
    let loaded = vec![Loaded {
        renderer: Box::new(crate::testing::Screen(vec2(200.0, 100.0))),
        fonts: None,
    }];
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");
    runner.frame(&mut Nowhere, SafeArea::default());
    framed.borrow_mut().clear();
    runner.reports().resized(0, 0);
    for _ in 0..3 {
        runner.frame(&mut Nowhere, SafeArea::default());
    }
    framed.take()
}

#[test]
fn a_paced_host_goes_on_while_its_window_is_minimised() {
    let _root = crate::reactive::RootScope::new();
    assert_eq!(
        frames_while_minimised(true),
        vec![vec2(200.0, 100.0); 3],
        "a headset's frames go on at the window's last size while it has none"
    );
    assert_eq!(
        frames_while_minimised(false),
        Vec::<Vec2>::new(),
        "a host that does not pace waits for the window to come back"
    );
}
