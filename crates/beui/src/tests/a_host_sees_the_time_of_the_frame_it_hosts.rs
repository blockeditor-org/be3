use super::*;
use beui_core::app::SafeArea;
use beui_core::context::FrameOutput;
use beui_core::renderer::Loaded;
use beui_core::runner::{
    HostView, Hosted, Incoming, Launch, PlatformServices, RunOptions, Runner, WindowHost,
};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Seen {
    Input(Instant),
    Frame(Instant),
    App(Instant),
}

type Log = Rc<RefCell<Vec<Seen>>>;

struct Nowhere;

impl PlatformServices for Nowhere {}

struct Recording(Log);

impl WindowHost for Recording {
    fn input(&mut self, _incoming: &mut Vec<Incoming>, view: &HostView) {
        self.0.borrow_mut().push(Seen::Input(view.now));
    }

    fn frame(&mut self, output: &FrameOutput, view: &HostView) -> Hosted {
        self.0.borrow_mut().push(Seen::Frame(view.now));
        Hosted {
            window: output.main().clone(),
            repaint_after: Duration::MAX,
        }
    }
}

struct Clocked(Log);

impl App for Clocked {
    fn update(&mut self, context: &Context, _rect: Rect) {
        self.0.borrow_mut().push(Seen::App(context.now()));
    }
}

#[test]
fn a_host_sees_the_time_of_the_frame_it_hosts() {
    let _root = crate::reactive::RootScope::new();
    let log = Log::default();
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Clocked(log.clone())),
        host: Some(Box::new(Recording(log.clone()))),
    });
    let loaded = vec![Loaded {
        renderer: Box::new(crate::testing::Screen(vec2(200.0, 100.0))),
        fonts: None,
    }];
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");
    for _ in 0..3 {
        runner.frame(&mut Nowhere, SafeArea::default());
        let seen = std::mem::take(&mut *log.borrow_mut());
        let [Seen::Input(input), Seen::App(app), Seen::Frame(frame)] = seen[..] else {
            panic!("the host and the app each see the frame once, in order: {seen:?}");
        };
        assert_eq!(
            input, app,
            "the host's input sees this frame's time, not the last frame's"
        );
        assert_eq!(frame, app, "the host's frame sees this frame's time");
    }
}
