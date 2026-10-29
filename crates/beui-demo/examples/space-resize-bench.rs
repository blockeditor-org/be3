#[cfg(not(target_arch = "wasm32"))]
mod bench {
    use std::any::Any;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use beui::datetime::Date;
    use beui::native::Document;
    use beui_host::native::testing::Screen;
    use beui::reactive::view;
    use beui::{
        Input, PointerButton, Pos2, Vec2, WindowId, native::Context, native::Incoming,
        native::Launch, native::PlatformServices, native::RunOptions, native::Setup, native::Waker,
    };
    use beui_core::app::SafeArea;
    use beui_core::renderer::{Loaded, Renderer};
    use beui_core::runner::Runner;
    use beui_core::window::WindowEdge;
    use beui_demo::DemoRoot;
    use beui_layer_space::Space;
    use beui_renderer_wgpu::offscreen::OffscreenSurface;

    const PLATFORM: Vec2 = Vec2::new(1440.0, 900.0);
    const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
    const STEPS: usize = 60;

    struct Bare;

    impl PlatformServices for Bare {}

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
            let context: Context = beui_host::native::context();
            context.stop_clock();
            let space = Rc::new(RefCell::new(Space::new()));
            let mut runner = Runner::new(Launch {
                options: RunOptions {
                    size: PLATFORM,
                    ..RunOptions::default()
                },
                context,
                app: Box::new(beui_host::native::build(|| {
                    view! {
                        <DemoRoot today={Date::today()} />
                    }
                })),
                layers: vec![Box::new(Rc::clone(&space))],
            });
            let renderer: Box<dyn Renderer> = match gpu {
                true => Box::new(
                    pollster::block_on(OffscreenSurface::new(
                        PLATFORM.x as u32,
                        PLATFORM.y as u32,
                        Default::default(),
                    ))
                    .expect("an offscreen GPU"),
                ),
                false => Box::new(Screen(PLATFORM)),
            };
            runner
                .start(
                    Loaded {
                        renderer,
                        fonts: None,
                    },
                    Setup::new(Waker::new(|| {})),
                )
                .expect("the bench's renderer starts");
            Self { runner, space }
        }

        fn app(&self) -> &Document {
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
            let latest = self.app().performance().latest;
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

        fn on_screen(&self, window: WindowId, pos: Pos2) -> Pos2 {
            let space = self.space.borrow();
            let point = space
                .panel(window)
                .expect("the window has a panel")
                .world(pos);
            let clip = space.head.view_projection(PLATFORM).project_point3(point);
            Pos2::new(
                (clip.x + 1.0) / 2.0 * PLATFORM.x,
                (1.0 - clip.y) / 2.0 * PLATFORM.y,
            )
        }

        fn docking_tab(&self) -> Pos2 {
            let document = self.app();
            let close = (0..64)
                .find_map(|tab| document.find_test_id(&format!("dock.tab.{tab}.close")))
                .expect("the Docking page is open in a tab");
            let rect = document.node_rect(close).expect("its tab is laid out");
            rect.center() - Vec2::new(50.0, 0.0)
        }

        fn float(&mut self) -> WindowId {
            let tab = self.on_screen(WindowId::MAIN, self.docking_tab());
            let sky = Pos2::new(PLATFORM.x / 2.0, 30.0);
            self.drag_to(tab, sky);
            let space = self.space.borrow();
            let window = space
                .panels
                .iter()
                .map(|panel| panel.window)
                .find(|window| !window.is_main())
                .expect("the tab floated into a window of its own");
            window
        }

        fn drag_to(&mut self, from: Pos2, to: Pos2) {
            let button = |pos, pressed| Input::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
            };
            self.frame(vec![Input::PointerMoved(from), button(from, true)]);
            for step in 1..=8 {
                let at = from + (to - from) * (step as f32 / 8.0);
                self.frame(vec![Input::PointerMoved(at)]);
            }
            self.frame(vec![button(to, false)]);
            self.settle();
        }

        fn resize(&mut self, window: WindowId) -> Vec<Timed> {
            let size = self.space.borrow().panel(window).unwrap().size;
            let corner = size - Vec2::new(2.0, 2.0);
            let grip = self.on_screen(window, Pos2::new(corner.x, corner.y));
            self.frame(vec![
                Input::PointerMoved(grip),
                Input::PointerButton {
                    pos: grip,
                    button: PointerButton::Primary,
                    pressed: true,
                },
            ]);
            if self.space.borrow().mouse.resize.is_none() {
                self.space.borrow_mut().begin_resize(
                    beui_layer_space::Pointer::Mouse,
                    window,
                    WindowEdge::BottomRight,
                );
            }
            let mut frames = Vec::new();
            for step in 1..=STEPS {
                let wave = (step as f32 / STEPS as f32 * std::f32::consts::TAU).sin();
                let at = grip + Vec2::new(wave * 120.0 + step as f32, wave * 60.0);
                frames.push(self.frame(vec![Input::PointerMoved(at)]));
            }
            let end = grip + Vec2::new(STEPS as f32, 0.0);
            self.frame(vec![Input::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
            }]);
            self.settle();
            frames
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
        let docked = bench.resize(WindowId::MAIN);
        report("main window, page docked", &docked);
        let window = bench.float();
        let floating = bench.resize(window);
        report("floating Docking window", &floating);
        let main = bench.resize(WindowId::MAIN);
        report("main window, page floating", &main);
        let scales: Vec<_> = {
            let space = bench.space.borrow();
            space
                .panels
                .iter()
                .map(|panel| (panel.window, panel.size, space.scale(panel.window)))
                .collect()
        };
        println!("panels {scales:?}");
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    bench::main();
}
