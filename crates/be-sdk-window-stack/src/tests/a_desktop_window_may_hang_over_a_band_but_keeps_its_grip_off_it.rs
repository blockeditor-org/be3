use super::*;

#[test]
fn a_desktop_window_may_hang_over_a_band_but_keeps_its_grip_off_it() {
    let display = Rect::from_min_size(pos2(800.0, 0.0), vec2(800.0, 600.0));
    let banded = Rect::from_min_max(pos2(800.0, 0.0), pos2(1600.0, 560.0));
    let areas = [(display, banded)];
    let low = Rect::from_min_size(pos2(1000.0, 450.0), vec2(300.0, 200.0));
    assert_eq!(
        reachable(low, &areas),
        low.min,
        "a window may hang over the display's foot band while its grip is above it"
    );
    let sunk = Rect::from_min_size(pos2(1000.0, 540.0), vec2(300.0, 200.0));
    assert_eq!(
        reachable(sunk, &areas),
        pos2(1000.0, 560.0 - KEPT.y),
        "but its grip is lifted clear of the band"
    );
    let left = Rect::from_min_max(pos2(840.0, 0.0), pos2(1600.0, 600.0));
    let across = Rect::from_min_size(pos2(820.0, 100.0), vec2(300.0, 200.0));
    assert_eq!(
        reachable(across, &[(display, left)]),
        pos2(840.0, 100.0),
        "a band down the left edge pushes the grip off it"
    );
    let beside = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let spanning = Rect::from_min_size(pos2(700.0, 100.0), vec2(300.0, 200.0));
    assert_eq!(
        reachable(spanning, &[(beside, beside), (display, banded)]),
        spanning.min,
        "an edge toward another display holds nothing back"
    );
}
