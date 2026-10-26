use super::*;
use beui_core::app::SafeArea;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, PlatformServices, RunOptions, Runner};

type Log = Signal<Vec<&'static str>>;

struct Screen(Log);

impl Renderer for Screen {
    fn name(&self) -> &'static str {
        "screen"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(200.0, 100.0))
    }

    fn prepare(&mut self, output: &crate::WindowOutput, _scale: f32, _background: Color32) -> bool {
        output.changed
    }

    fn present(&mut self, _background: Color32) -> bool {
        self.0.update(|log| log.push("presented"));
        false
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.0.update(|log| log.push("renderer dropped"));
    }
}

struct Nowhere;

impl PlatformServices for Nowhere {}

struct Holding(Log);

impl App for Holding {
    fn update(&mut self, _context: &Context, _rect: Rect) {
        self.0.update(|log| log.push("updated"));
    }

    fn exiting(&mut self) {
        self.0.update(|log| log.push("exiting"));
    }
}

impl Drop for Holding {
    fn drop(&mut self) {
        self.0.update(|log| log.push("app dropped"));
    }
}

#[test]
fn an_exiting_runner_lets_its_app_go_while_its_renderers_are_still_there() {
    let _root = crate::reactive::RootScope::new();
    let log = create_signal(Vec::new());
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(Holding(log)),
        host: None,
    });
    let loaded = vec![Loaded {
        renderer: Box::new(Screen(log)),
        fonts: None,
    }];
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");
    runner
        .frame(&mut Nowhere, SafeArea::default())
        .expect("a loaded runner draws");
    runner.present();

    runner.exit();
    assert_eq!(
        log.update(std::mem::take),
        ["updated", "presented", "exiting", "app dropped"]
    );
    runner.present();
    assert!(
        log.update(std::mem::take).is_empty(),
        "an exited runner presents nothing more"
    );
    assert!(
        runner.frame(&mut Nowhere, SafeArea::default()).is_none(),
        "an exited runner draws nothing more"
    );
    runner.exit();
    assert!(
        log.update(std::mem::take).is_empty(),
        "the app hears that it exits once"
    );

    drop(runner);
    assert_eq!(log.update(std::mem::take), ["renderer dropped"]);
}
