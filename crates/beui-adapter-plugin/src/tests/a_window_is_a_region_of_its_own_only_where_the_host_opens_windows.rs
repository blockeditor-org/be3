use std::cell::RefCell;

use beui::primitives::Window;
use beui::reactive::*;
use beui::styled;
use beui::styled::*;
use beui_core::document::Document;
use beui_core::window::WindowPlacement;

use super::*;

struct Shared(Rc<RefCell<Document>>);

impl App for Shared {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.borrow_mut().show(context, rect);
    }
}

#[test]
fn a_window_is_a_region_of_its_own_only_where_the_host_opens_windows() {
    let _root = RootScope::new();
    let window = WindowId::next();
    let document = Rc::new(RefCell::new(build(move || {
        view! {
            <styled::Root>
                <Window
                    id={Some(window)}
                    size={vec2(120.0, 80.0)}
                    on_report={|_: WindowReport| {}}
                >|_: beui::primitives::WindowHandle|
                    <Text>"Inspector"</Text>
                </Window>
            </styled::Root>
        }
    })));
    let mut surface = PluginSurface::launch(
        EditorHost::default(),
        beui::context(),
        Shared(Rc::clone(&document)),
    );
    let whole = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
    let mut framed = region(whole, [400, 300]);
    for _ in 0..3 {
        assert_eq!(surface.update(&framed).window_requests, Vec::new());
    }

    framed.opens_windows = true;
    let asked: Vec<WindowRequest> = (0..3)
        .flat_map(|_| surface.update(&framed).window_requests)
        .collect();
    assert_eq!(
        asked,
        vec![WindowRequest::Open {
            window,
            placement: WindowPlacement::AtPointer,
            size: vec2(120.0, 80.0),
        }],
        "the window is asked for once the host opens windows"
    );

    let shown = Region {
        region: EditorRegion::Window(window),
        scale_factor: 2.0,
        ..region(Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 80.0)), [240, 160])
    };
    let first = surface.update_window(window, &shown);
    assert_eq!(
        first.repaint_after,
        Some(std::time::Duration::ZERO),
        "the window's size and scale reach the document on the next frame"
    );
    surface.update(&framed);
    let drawn = surface.update_window(window, &shown);
    assert!(drawn.changed, "the window's own output is drawn on its region");
    let painting = surface
        .window_painting(window)
        .expect("the window paints on its own");
    assert_eq!(painting.frame.pixels_per_point, 2.0);
    assert_eq!(painting.frame.size, [240, 160]);
}
