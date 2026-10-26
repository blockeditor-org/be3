use super::*;
use crate::primitives::{Window, WindowHandle};
use crate::{WindowId, WindowReport};
use beui_core::app::SafeArea;
use beui_core::renderer::{Loaded, Renderer};
use beui_core::runner::{Launch, RunOptions, Runner};

struct Screen;

impl Renderer for Screen {
    fn name(&self) -> &'static str {
        "screen"
    }

    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(400.0, 300.0))
    }

    fn prepare(
        &mut self,
        _output: &crate::WindowOutput,
        _scale: f32,
        _background: Color32,
    ) -> bool {
        false
    }

    fn present(&mut self, _background: Color32) -> bool {
        false
    }
}

struct Nowhere;

impl beui_core::runner::PlatformServices for Nowhere {}

#[test]
fn the_space_host_opens_an_apps_window_as_a_panel_at_the_pointer_and_clicks_reach_it() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let heard = create_signal(Vec::<WindowReport>::new());
    let clicks = create_signal(0);
    let document = build_node(move || {
        view! {
            <Window
                id={window}
                size={vec2(200.0, 100.0)}
                on_report={move |report: WindowReport| heard.update(|heard| heard.push(report))}
            >|_: WindowHandle|
                <Interactive on_click={move || clicks.update(|clicks| *clicks += 1)}>
                    <Frame width=200.0 height=100.0 />
                </Interactive>
            </Window>
        }
    });
    let context = context();
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context.clone(),
        app: Box::new(crate::testing::Showing(document)),
        host: Some(Box::new(beui_layer_space::Space::new())),
    });
    runner
        .start(
            vec![Loaded {
                renderer: Box::new(Screen),
                fonts: None,
            }],
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");
    for _ in 0..3 {
        runner.frame(&mut Nowhere, SafeArea::default());
    }

    let heard = heard.get_untracked();
    assert!(
        heard.contains(&WindowReport::Resized {
            window,
            size: vec2(200.0, 100.0)
        }),
        "the host opens the window at its size: {heard:?}"
    );
    let placement = context
        .window_placement(window)
        .expect("the host places the window");
    assert!(
        placement.0.starts_with("space "),
        "at a pose of its own: {placement:?}"
    );

    let middle = pos2(200.0, 150.0);
    runner.push(Input::PointerMoved(middle));
    for pressed in [true, false] {
        runner.push(Input::PointerButton {
            pos: middle,
            button: PointerButton::Primary,
            pressed,
        });
    }
    runner.frame(&mut Nowhere, SafeArea::default());
    assert_eq!(
        clicks.get_untracked(),
        1,
        "a click where the window opened, at the pointer, reaches it"
    );
}
