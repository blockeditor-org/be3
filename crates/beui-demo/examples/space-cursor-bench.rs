#[cfg(not(target_arch = "wasm32"))]
mod bench {
    use std::any::Any;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use beui::testing::Screen;
    use beui::{
        Context, CursorIcon, Incoming, Input, Launch, PlatformServices, Pos2, RunOptions, Setup,
        Vec2, Waker,
    };
    use beui_core::app::SafeArea;
    use beui_core::renderer::{Loaded, Renderer};
    use beui_core::runner::{HostView, Hosted, Runner, WindowHost};
    use beui_demo::DemoApp;
    use beui_layer_space::Space;
    use beui_renderer_wgpu::offscreen::OffscreenSurface;

    const PLATFORM: Vec2 = Vec2::new(1440.0, 900.0);
    const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
    const STEPS: usize = 60;

    struct Bare;

    impl PlatformServices for Bare {}

    struct Shared(Rc<RefCell<Space>>);

    impl WindowHost for Shared {
        fn setup(&mut self, setup: &Setup) {
            self.0.borrow_mut().setup(setup);
        }

        fn input(&mut self, incoming: &mut Vec<Incoming>, view: &HostView) {
            self.0.borrow_mut().input(incoming, view);
        }

        fn frame(&mut self, output: &beui_core::context::FrameOutput, view: &HostView) -> Hosted {
            self.0.borrow_mut().frame(output, view)
        }
    }

    struct Bench {
        runner: Runner,
        space: Rc<RefCell<Space>>,
    }

    #[derive(Default)]
    struct Timed {
        frame: Duration,
        present: Duration,
        layout: Duration,
        paint: Duration,
        placed: usize,
        painted: usize,
        changed: usize,
    }

    impl Bench {
        fn new(gpu: bool) -> Self {
            let context: Context = beui::context();
            context.stop_clock();
            let space = Rc::new(RefCell::new(Space::new()));
            let mut runner = Runner::new(Launch {
                options: RunOptions {
                    size: PLATFORM,
                    ..RunOptions::default()
                },
                context,
                app: Box::new(DemoApp::new()),
                host: Some(Box::new(Shared(Rc::clone(&space)))),
            });
            let renderer: Box<dyn Renderer> = match gpu {
                true => Box::new(
                    pollster::block_on(OffscreenSurface::new(
                        PLATFORM.x as u32,
                        PLATFORM.y as u32,
                        None,
                    ))
                    .expect("an offscreen GPU"),
                ),
                false => Box::new(Screen(PLATFORM)),
            };
            runner
                .start(
                    vec![Loaded {
                        renderer,
                        fonts: None,
                    }],
                    Setup::new(Waker::new(|| {})),
                )
                .expect("the bench's renderer starts");
            Self { runner, space }
        }

        fn app(&self) -> &DemoApp {
            let app: &dyn Any = self.runner.app();
            app.downcast_ref().expect("the bench runs the demo")
        }

        fn frame(&mut self, inputs: Vec<Input>) -> Timed {
            self.runner.context().advance_clock(FRAME_INTERVAL);
            for input in inputs {
                self.runner.push(Incoming::from(input));
            }
            let started = Instant::now();
            self.runner.frame(&mut Bare, SafeArea::default());
            let frame = started.elapsed();
            if let Some(failure) = self.runner.failure() {
                panic!("the demo panicked: {failure}");
            }
            let started = Instant::now();
            self.runner.present();
            let present = started.elapsed();
            let changed = self.runner.output().map_or(0, |output| {
                output
                    .windows
                    .iter()
                    .filter(|window| window.changed)
                    .count()
            });
            let latest = self.app().document.performance().latest;
            Timed {
                frame,
                present,
                layout: latest.timings.layout,
                paint: latest.timings.paint,
                placed: latest.work.placed,
                painted: latest.work.painted_nodes,
                changed,
            }
        }

        fn settle(&mut self) {
            for _ in 0..60 {
                self.frame(Vec::new());
                let again = self
                    .runner
                    .output()
                    .is_some_and(|output| output.repaint_after.is_zero());
                if !again {
                    break;
                }
            }
        }

        fn icon(&self) -> CursorIcon {
            let output = self.runner.output().expect("the bench has framed");
            self.space.borrow().cursor_icon(output)
        }

        fn find_icons(&mut self) -> (Pos2, Pos2) {
            let mut seen: Vec<(CursorIcon, Pos2)> = Vec::new();
            for row in 1..30 {
                for column in 1..40 {
                    let at = Pos2::new(
                        PLATFORM.x * column as f32 / 40.0,
                        PLATFORM.y * row as f32 / 30.0,
                    );
                    self.frame(vec![Input::PointerMoved(at)]);
                    self.settle();
                    let icon = self.icon();
                    if !seen.iter().any(|(known, _)| *known == icon) {
                        seen.push((icon, at));
                    }
                    if let Some(&(_, other)) = seen.iter().find(|(known, _)| *known != seen[0].0) {
                        println!(
                            "icons found {:?}",
                            seen.iter().map(|(icon, _)| *icon).collect::<Vec<_>>()
                        );
                        return (seen[0].1, other);
                    }
                }
            }
            panic!("the demo shows only one cursor icon: {seen:?}");
        }

        fn switch(&mut self, from: Pos2, to: Pos2) -> Vec<Timed> {
            (0..STEPS)
                .map(|step| {
                    let at = match step % 2 {
                        0 => to,
                        _ => from,
                    };
                    self.frame(vec![Input::PointerMoved(at)])
                })
                .collect()
        }
    }

    fn report(label: &str, frames: &[Timed]) {
        let median = |pick: &dyn Fn(&Timed) -> Duration| {
            let mut all: Vec<Duration> = frames.iter().map(pick).collect();
            all.sort();
            (all[all.len() / 2], all[all.len() - 1])
        };
        let (total, worst) = median(&|timed| timed.frame + timed.present);
        let (frame, _) = median(&|timed| timed.frame);
        let (present, _) = median(&|timed| timed.present);
        let (layout, _) = median(&|timed| timed.layout);
        let (paint, _) = median(&|timed| timed.paint);
        let placed = frames.iter().map(|timed| timed.placed).max().unwrap_or(0);
        let painted = frames.iter().map(|timed| timed.painted).max().unwrap_or(0);
        let changed = frames.iter().map(|timed| timed.changed).max().unwrap_or(0);
        if std::env::var_os("VERBOSE").is_some() {
            for timed in frames {
                println!(
                    "  frame {:>9.2?} present {:>9.2?} placed {:>5} painted {:>5} windows changed {}",
                    timed.frame, timed.present, timed.placed, timed.painted, timed.changed
                );
            }
        }
        println!(
            "{label:<28} median {total:>9.2?} worst {worst:>9.2?} (frame {frame:>9.2?} present {present:>9.2?} layout {layout:>8.2?} paint {paint:>8.2?}) most placed {placed:>5} painted {painted:>5} windows changed {changed}"
        );
    }

    pub fn main() {
        let gpu = std::env::var_os("NO_GPU").is_none();
        let mut bench = Bench::new(gpu);
        bench.settle();
        bench.space.borrow_mut().head.position.z += 1.5;
        bench.settle();
        let (first, second) = bench.find_icons();
        let nudge = first + Vec2::new(1.0, 0.0);
        bench.frame(vec![Input::PointerMoved(first)]);
        bench.settle();
        let same = bench.switch(first, nudge);
        report("moving, same icon", &same);
        let switching = bench.switch(first, second);
        report("switching icon every frame", &switching);
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    bench::main();
}
