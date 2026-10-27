use super::*;
use beui_core::app::SafeArea;
use beui_core::context::FrameOutput;
use beui_core::renderer::Loaded;
use beui_core::runner::{
    Frame, Incoming, Launch, Layer, LayerFrame, PlatformServices, RunOptions, Runner, Shown,
};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Seen {
    Input(Instant),
    Output(Instant),
    Framed(Instant),
    App(Instant),
}

type Log = Rc<RefCell<Vec<Seen>>>;

struct Nowhere;

impl PlatformServices for Nowhere {}

struct Recording(Log);

impl Layer for Recording {
    fn input(&mut self, _incoming: &mut Vec<Incoming>, frame: &mut LayerFrame<'_>) {
        self.0.borrow_mut().push(Seen::Input(frame.view.now));
    }

    fn output(&mut self, _shown: &mut Shown, _output: &FrameOutput, frame: &mut LayerFrame<'_>) {
        self.0.borrow_mut().push(Seen::Output(frame.view.now));
    }

    fn framed(&mut self, _frame: &mut Frame, _output: &FrameOutput, layer: &mut LayerFrame<'_>) {
        self.0.borrow_mut().push(Seen::Framed(layer.view.now));
    }
}

struct Clocked(Log);

impl App for Clocked {
    fn update(&mut self, context: &Context, _rect: Rect) {
        self.0.borrow_mut().push(Seen::App(context.now()));
    }
}

#[test]
fn a_layer_sees_the_time_of_the_frame_it_is_in() {
    let _root = crate::reactive::RootScope::new();
    let log = Log::default();
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Clocked(log.clone())),
        layers: vec![Box::new(Recording(log.clone()))],
    });
    let loaded = Loaded {
        renderer: Box::new(crate::native::testing::Screen(vec2(200.0, 100.0))),
        fonts: None,
    };
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");
    for _ in 0..3 {
        runner.frame(&mut Nowhere, SafeArea::default());
        let seen = std::mem::take(&mut *log.borrow_mut());
        let [
            Seen::Input(input),
            Seen::App(app),
            Seen::Output(output),
            Seen::Framed(framed),
        ] = seen[..]
        else {
            panic!("the layer and the app each see the frame once, in order: {seen:?}");
        };
        assert_eq!(
            input, app,
            "the layer's input sees this frame's time, not the last frame's"
        );
        assert_eq!(output, app, "the layer's output sees this frame's time");
        assert_eq!(framed, app, "the layer's timing sees this frame's time");
    }
}
