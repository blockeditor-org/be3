use be_sdk_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_window_dragged_over_the_bars_passes_beneath_them() {
    let display = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 600.0));
    let mut fixture = Fixture::new();
    fixture.test.show_on_displays(&[(display, 1.0)]);
    fixture.settle();
    fixture.show_windows(&[host_window(1, true)]);
    fixture.settle();
    let band = fixture
        .test
        .desk_rects_of("dock.bar.band.bottom")
        .first()
        .copied()
        .expect("the desktop has a band at its foot");
    let raised = |fixture: &Fixture| {
        let document = fixture.test.document();
        let node = document
            .find_test_id("dock.bar.band.bottom")
            .expect("the band is shown");
        let window = document.window_of(node);
        let rect = document.node_rect(node).expect("the band is laid out");
        let painted = document.overlay_painted_rects(window);
        (
            painted
                .iter()
                .any(|held| held.contains_rect(rect.shrink(1.0))),
            painted,
        )
    };
    let (above, painted) = raised(&fixture);
    assert!(
        above,
        "the shell's view reports its bars' band as drawn above the windows it hosts: {band:?} {painted:?}"
    );

    let grab = fixture.window_grip(1);
    let to = pos2(grab.x, band.top() - 120.0);
    fixture.test.desk_press(grab);
    fixture
        .test
        .desk_moves(grab, to, 8, std::time::Duration::ZERO);
    fixture.settle();
    let window = fixture.test.desk_windows()[0];
    assert!(
        window.intersects(band),
        "held, the window is carried over the bars: {window:?} {band:?}"
    );
    fixture
        .test
        .desk_snapshot("a_window_dragged_over_the_bars_passes_beneath_them", 0);
    fixture.test.desk_release(to);
    fixture.settle();

    fixture.show_windows(&[HostWindow {
        fullscreen: Some(display),
        ..host_window(1, true)
    }]);
    fixture.settle();
    assert!(
        !raised(&fixture).0,
        "a fullscreen window covers the bars, so they are no longer drawn above it"
    );
}
