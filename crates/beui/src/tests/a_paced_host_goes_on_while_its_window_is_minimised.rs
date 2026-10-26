use super::*;
use beui_core::app::SafeArea;
use beui_core::context::FrameOutput;
use beui_core::renderer::Loaded;
use beui_core::runner::{Frame, Launch, Layer, LayerFrame, PlatformServices, RunOptions, Runner};
use std::time::Instant;

struct Nowhere;

impl PlatformServices for Nowhere {}

struct Paced {
    paces: bool,
    framed: Rc<RefCell<Vec<Vec2>>>,
}

impl Layer for Paced {
    fn pace(&mut self) -> Option<Instant> {
        self.paces.then(Instant::now)
    }

    fn framed(&mut self, frame: &mut Frame, _output: &FrameOutput, layer: &mut LayerFrame<'_>) {
        self.framed.borrow_mut().push(layer.view.platform);
        frame.repaint_after = Duration::ZERO;
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
        layers: vec![Box::new(Paced {
            paces,
            framed: framed.clone(),
        })],
    });
    let loaded = Loaded {
        renderer: Box::new(crate::testing::Screen(vec2(200.0, 100.0))),
        fonts: None,
    };
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
