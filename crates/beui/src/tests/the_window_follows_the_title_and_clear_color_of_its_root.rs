use super::*;
use beui_core::app::SafeArea;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, PlatformServices, RunOptions, Runner};
use beui_core::window::WindowState;

struct Screen(Rc<RefCell<Vec<Color32>>>);

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

    fn prepare(&mut self, _output: &crate::WindowOutput, _scale: f32, background: Color32) -> bool {
        self.0.borrow_mut().push(background);
        false
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

struct Titles(Vec<String>);

impl PlatformServices for Titles {
    fn show_window(&mut self, window: &WindowState, shown: &WindowState) {
        if window.appearance.title != shown.appearance.title
            && let Some(title) = &window.appearance.title
        {
            self.0.push(title.clone());
        }
    }
}

#[test]
fn the_window_follows_the_title_and_clear_color_of_its_root() {
    let _root = crate::reactive::RootScope::new();
    let title = create_signal("First".to_owned());
    let clear = create_signal(Color32::BLACK);
    let document = build(move || {
        view! {
            <beui_view::primitives::Root title={title} clear_color={clear}>
                <Frame />
            </beui_view::primitives::Root>
        }
    });
    let backgrounds = Rc::new(RefCell::new(Vec::new()));
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(crate::testing::Showing(document)),
        layers: Vec::new(),
    });
    let loaded = vec![Loaded {
        renderer: Box::new(Screen(Rc::clone(&backgrounds))),
        fonts: None,
    }];
    runner
        .start(loaded, Setup::new(Waker::new(|| {})))
        .expect("the renderer loads");
    let mut platform = Titles(Vec::new());

    runner.frame(&mut platform, SafeArea::default());
    assert_eq!(platform.0, ["First"]);
    assert_eq!(backgrounds.take(), [Color32::BLACK]);

    title.set("Second".to_owned());
    clear.set(Color32::WHITE);
    runner.frame(&mut platform, SafeArea::default());
    assert_eq!(platform.0, ["First", "Second"]);
    assert_eq!(backgrounds.take(), [Color32::WHITE]);
}
