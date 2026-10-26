use beui_core::window::WindowRole;

use super::*;

#[test]
fn a_window_is_a_region_of_its_own_only_where_the_host_opens_windows() {
    let _root = RootScope::new();
    let window = WindowId::next();
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            view! {
                <Window
                    id={Some(window)}
                    size={vec2(120.0, 80.0)}
                    on_report={|_: WindowReport| {}}
                >|_: WindowHandle|
                    <styled::Frame fill=styled::ColorRole::Accent />
                </Window>
            }
        }),
    );
    let mut framed = whole(400.0, 300.0);
    for _ in 0..3 {
        assert_eq!(one(&mut surface, &framed).window_requests, Vec::new());
    }

    framed.opens_windows = true;
    let asked: Vec<WindowRequest> = (0..3)
        .flat_map(|_| one(&mut surface, &framed).window_requests)
        .collect();
    let opened = match asked.as_slice() {
        [
            WindowRequest::Open {
                window: opened,
                size,
                role: WindowRole::Normal,
                ..
            },
        ] => {
            assert_eq!(*size, vec2(120.0, 80.0));
            *opened
        }
        asked => panic!("the window is asked for once the host opens windows: {asked:?}"),
    };
    assert_eq!(opened, window);

    let shown = Region {
        region: EditorRegion::Window(window),
        scale_factor: 2.0,
        raster: 2.0,
        ..region(vec2(120.0, 80.0), Vec2::ZERO, [240, 160])
    };
    let drawn = surface.frame(&[framed.clone(), shown.clone()]);
    assert!(
        drawn[1].changed,
        "the window's own output is drawn on its region"
    );
    assert_eq!(
        surface.window(shown.id()),
        Some(window),
        "the window's region is the window itself, not a window opened for it"
    );
    let painting = surface
        .painting(shown.id())
        .expect("the window paints on its own");
    assert_eq!(painting.frame.pixels_per_point, 2.0);
    assert_eq!(painting.frame.size, [240, 160]);
}
