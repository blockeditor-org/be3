#[cfg(not(target_arch = "wasm32"))]
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicU64, Ordering};

    static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

    struct Counting;

    // unsafe(memory)
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            unsafe { System.realloc(ptr, layout, size) }
        }
    }

    #[global_allocator]
    static GLOBAL: Counting = Counting;

    pub fn allocations() -> u64 {
        ALLOCATIONS.load(Ordering::Relaxed)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod bench {
    use std::any::Any;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use beui::testing::Screen;
    use beui::{
        Color32, Context, Incoming, Input, Launch, PlatformServices, PointerButton, Pos2, Rect,
        RunOptions, ScrollDelta, Setup, Vec2, Waker, WindowOutput,
    };
    use beui_core::app::SafeArea;
    use beui_core::context::RendererInfo;
    use beui_core::renderer::{Loaded, Renderer};
    use beui_core::runner::Runner;
    use beui_demo::DemoApp;
    use beui_renderer_wgpu::offscreen::OffscreenSurface;

    const VIEWPORT: Vec2 = Vec2::new(1440.0, 900.0);
    const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
    const SETTLE_LIMIT: usize = 120;
    const PAGES: [&str; 27] = [
        "Text",
        "Buttons",
        "Inputs",
        "Choices",
        "Pickers",
        "Colors",
        "Menus",
        "Overlays",
        "Rows",
        "Tree",
        "Layout",
        "Editor",
        "Collaboration",
        "Canvas",
        "Themes",
        "Pressing",
        "Values",
        "Selecting",
        "Popups",
        "Dragging",
        "Scrolling",
        "Frames",
        "Text nodes",
        "Lists and grids",
        "Control flow",
        "Interaction",
        "Layering",
    ];

    type Gpu = Rc<RefCell<Option<OffscreenSurface>>>;

    struct Bare;

    impl PlatformServices for Bare {}

    struct Timing {
        gpu: Gpu,
        screen: Screen,
        prepared: Rc<Cell<Duration>>,
    }

    impl Renderer for Timing {
        fn info(&self) -> RendererInfo {
            RendererInfo::default()
        }

        fn resize(&mut self, width: u32, height: u32) {
            self.screen.resize(width, height);
        }

        fn physical(&self) -> Option<Vec2> {
            self.screen.physical()
        }

        fn prepare(&mut self, output: &WindowOutput, scale: f32, background: Color32) -> bool {
            let started = Instant::now();
            let pending = match &mut *self.gpu.borrow_mut() {
                Some(gpu) => gpu.prepare(output, scale, background),
                None => self.screen.prepare(output, scale, background),
            };
            self.prepared.set(started.elapsed());
            pending
        }

        fn present(&mut self, background: Color32) -> bool {
            match &mut *self.gpu.borrow_mut() {
                Some(gpu) => gpu.present(background),
                None => false,
            }
        }
    }

    struct Bench {
        runner: Runner,
        gpu: Gpu,
        prepared: Rc<Cell<Duration>>,
    }

    struct Timed {
        took: Duration,
        prepare: Duration,
        present: Duration,
        repaint: bool,
        perf: beui::PerformanceSnapshot,
    }

    impl Bench {
        fn new(context: Context, gpu: Gpu) -> Self {
            let prepared = Rc::new(Cell::new(Duration::ZERO));
            let mut runner = Runner::new(Launch {
                options: RunOptions {
                    size: VIEWPORT,
                    ..RunOptions::default()
                },
                context,
                app: Box::new(DemoApp::new()),
                layers: Vec::new(),
            });
            let timing = Timing {
                gpu: Rc::clone(&gpu),
                screen: Screen(VIEWPORT),
                prepared: Rc::clone(&prepared),
            };
            runner
                .start(
                    Loaded {
                        renderer: Box::new(timing),
                        fonts: None,
                    },
                    Setup::new(Waker::new(|| {})),
                )
                .expect("the bench's renderer starts");
            Self {
                runner,
                gpu,
                prepared,
            }
        }

        fn app(&self) -> &DemoApp {
            let app: &dyn Any = self.runner.app();
            app.downcast_ref().expect("the bench runs the demo")
        }

        fn frame(&mut self, inputs: Vec<Input>) -> Timed {
            self.runner.context().advance_clock(FRAME_INTERVAL);
            for input in inputs {
                self.runner.push(Incoming::points(input));
            }
            self.prepared.set(Duration::ZERO);
            let started = Instant::now();
            self.runner.frame(&mut Bare, SafeArea::default());
            let elapsed = started.elapsed();
            if let Some(failure) = self.runner.failure() {
                panic!("the demo panicked: {failure}");
            }
            let prepare = self.prepared.get();
            let took = elapsed.saturating_sub(prepare);
            let mut present = Duration::ZERO;
            if self.gpu.borrow().is_some() {
                let started = Instant::now();
                self.runner.present();
                present = started.elapsed();
            }
            let repaint = self
                .runner
                .output()
                .expect("a running bench draws every frame")
                .repaint;
            Timed {
                took,
                prepare,
                present,
                repaint,
                perf: self.app().document.performance(),
            }
        }

        fn settle(&mut self) -> Vec<Timed> {
            let mut frames = Vec::new();
            for _ in 0..SETTLE_LIMIT {
                let timed = self.frame(Vec::new());
                let repaint = timed.repaint;
                frames.push(timed);
                if !repaint {
                    break;
                }
            }
            frames
        }

        fn row(&self, title: &str) -> Rect {
            let document = &self.app().document;
            let node = document
                .find_test_id(&format!("demo.catalog.{title}"))
                .unwrap_or_else(|| panic!("no catalog row for {title}"));
            document.node_rect(node).expect("the row was laid out")
        }

        fn open(&mut self, title: &str) -> (Vec<Timed>, u64) {
            let catalog = self.row("Text").center();
            for _ in 0..40 {
                let rect = self.row(title);
                if rect.max.y < VIEWPORT.y - 40.0 && rect.min.y > 80.0 {
                    break;
                }
                let down = if rect.max.y >= VIEWPORT.y - 40.0 {
                    -120.0
                } else {
                    120.0
                };
                self.frame(vec![
                    Input::PointerMoved(catalog),
                    Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, down))),
                ]);
                self.frame(vec![Input::ScrollEnded]);
                self.settle();
            }
            let pos = self.row(title).center();
            self.frame(vec![Input::PointerMoved(pos)]);
            self.settle();
            self.frame(vec![Input::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
            }]);
            self.settle();
            self.release(pos)
        }

        #[inline(never)]
        fn release(&mut self, pos: Pos2) -> (Vec<Timed>, u64) {
            let allocations = crate::counting::allocations();
            let mut frames = vec![self.frame(vec![Input::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
            }])];
            frames.extend(self.settle());
            let allocations = crate::counting::allocations() - allocations;
            if std::env::var_os("UNPLACED").is_some() {
                let document = &self.app().document;
                let mut stack: Vec<_> = document.root().into_iter().collect();
                let mut found = std::collections::HashMap::<String, (usize, usize)>::new();
                fn size(document: &beui_core::document::Document, node: beui::NodeId) -> usize {
                    1 + document
                        .children(node)
                        .into_iter()
                        .map(|child| size(document, child))
                        .sum::<usize>()
                }
                while let Some(node) = stack.pop() {
                    if document.node_rect(node).is_none() {
                        let mut name = document.component_names(node).join("<");
                        let mut parent = document.arena.parent(node);
                        for _ in 0..4 {
                            let Some(at) = parent else { break };
                            if let Some(first) = document.component_names(at).first() {
                                name = format!("{name} in {first}");
                            }
                            parent = document.arena.parent(at);
                        }
                        let entry = found.entry(name).or_default();
                        entry.0 += 1;
                        entry.1 += size(document, node);
                        continue;
                    }
                    stack.extend(document.children(node));
                }
                let mut found: Vec<_> = found.into_iter().collect();
                found.sort_by_key(|(_, (_, nodes))| std::cmp::Reverse(*nodes));
                for (name, (roots, nodes)) in found.into_iter().take(25) {
                    println!("unplaced {nodes:5} nodes in {roots:3} roots: {name}");
                }
            }
            (frames, allocations)
        }
    }

    fn report(
        label: &str,
        title: &str,
        frames: &[Timed],
        allocations: u64,
        verbose: bool,
    ) -> Duration {
        let worst = frames
            .iter()
            .max_by_key(|timed| timed.took + timed.prepare)
            .expect("a frame");
        let total: Duration = frames.iter().map(|timed| timed.took).sum();
        let latest = worst.perf.latest;
        if !verbose {
            return worst.took + worst.prepare;
        }
        let prepare = frames
            .iter()
            .map(|timed| timed.prepare)
            .max()
            .unwrap_or_default();
        let present = frames
            .iter()
            .map(|timed| timed.present)
            .max()
            .unwrap_or_default();
        println!(
            "{label:<6} {title:<16} worst {:>8.2?} prepare {prepare:>8.2?} present {present:>8.2?} total {:>8.2?} frames {:>3}  [show {:>7.2?} interact {:>7.2?} layout {:>7.2?} paint {:>7.2?} a11y {:>7.2?} other {:>7.2?}] nodes {:>5} shapes {:>5} measured {:>5} placed {:>5} painted {:>5} allocations {allocations:>6}",
            worst.took,
            total,
            frames.len(),
            latest.timings.total,
            latest.timings.interaction,
            latest.timings.layout,
            latest.timings.paint,
            latest.timings.accessibility,
            latest.timings.other,
            latest.nodes,
            latest.shapes,
            latest.work.measured,
            latest.work.placed,
            latest.work.painted_nodes,
        );
        worst.took + worst.prepare
    }

    pub fn main() {
        let filter = std::env::args().nth(1);
        let context = beui::context();
        context.stop_clock();
        let started = Instant::now();
        let gpu = std::env::var_os("GPU").map(|_| {
            pollster::block_on(OffscreenSurface::new(
                VIEWPORT.x as u32,
                VIEWPORT.y as u32,
                Default::default(),
            ))
            .expect("an offscreen GPU")
        });
        let gpu: Gpu = Rc::new(RefCell::new(gpu));
        let mut bench = Bench::new(context.clone(), Rc::clone(&gpu));
        bench.frame(Vec::new());
        bench.settle();
        println!("startup {:?}", started.elapsed());
        let repeat: usize = std::env::var("REPEAT")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(10);
        let verbose = std::env::var_os("VERBOSE").is_some();
        let mut summary: Vec<Summary> = Vec::new();
        for round in 0..=repeat {
            let label = if round == 0 { "cold" } else { "warm" };
            for title in PAGES {
                if filter
                    .as_ref()
                    .is_some_and(|filter| !title.contains(filter.as_str()))
                {
                    continue;
                }
                let mut bench = Bench::new(context.clone(), Rc::clone(&gpu));
                bench.frame(Vec::new());
                bench.settle();
                let (frames, allocations) = bench.open(title);
                let worst = report(label, title, &frames, allocations, verbose);
                match summary.iter_mut().find(|held| held.title == title) {
                    Some(held) => {
                        held.warm.push(worst);
                        held.allocations = allocations;
                    }
                    None => summary.push(Summary {
                        title,
                        cold: worst,
                        warm: Vec::new(),
                        allocations,
                    }),
                }
            }
        }
        println!(
            "{:<16} {:>9} {:>9} {:>9} {:>9} {:>12}",
            "page", "cold", "warm min", "warm p50", "warm max", "allocations"
        );
        let ms = |duration: Duration| duration.as_secs_f64() * 1e3;
        let mut logs = [0.0; 4];
        for Summary {
            title,
            cold,
            mut warm,
            allocations,
        } in summary.iter().cloned()
        {
            warm.sort();
            let min = warm.first().copied().unwrap_or_default();
            let median = warm.get(warm.len() / 2).copied().unwrap_or_default();
            let max = warm.last().copied().unwrap_or_default();
            for (log, duration) in logs.iter_mut().zip([cold, min, median, max]) {
                *log += ms(duration).max(1e-3).ln();
            }
            let page = title.to_lowercase().replace(' ', "-");
            ::bench::time(&format!("{page}.cold"), cold);
            ::bench::time(&format!("{page}.warm-fastest"), min);
            ::bench::time(&format!("{page}.warm-median"), median);
            ::bench::count(&format!("{page}.allocations"), allocations);
            println!(
                "{title:<16} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {allocations:>12}",
                ms(cold),
                ms(min),
                ms(median),
                ms(max),
            );
        }
        let [cold, min, median, max] = logs.map(|log| (log / summary.len().max(1) as f64).exp());
        ::bench::record("mean.cold", cold, "ms");
        ::bench::record("mean.warm-fastest", min, "ms");
        ::bench::record("mean.warm-median", median, "ms");
        println!(
            "{:<16} {cold:>9.2} {min:>9.2} {median:>9.2} {max:>9.2}",
            "geomean"
        );
    }

    #[derive(Clone)]
    struct Summary {
        title: &'static str,
        cold: Duration,
        warm: Vec<Duration>,
        allocations: u64,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    bench::main();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
