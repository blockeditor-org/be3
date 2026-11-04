use super::*;

#[test]
fn a_desktop_window_is_kept_off_the_bands_of_its_display_and_free_across_others() {
    let display = Rect::from_min_size(pos2(800.0, 0.0), vec2(800.0, 600.0));
    let banded = Rect::from_min_max(pos2(800.0, 0.0), pos2(1600.0, 560.0));
    let low = Rect::from_min_size(pos2(1000.0, 450.0), vec2(300.0, 200.0));
    assert_eq!(
        walled(low, display, banded),
        pos2(1000.0, 360.0),
        "a window over the display's foot band is lifted clear of it"
    );
    let across = Rect::from_min_size(pos2(700.0, 100.0), vec2(300.0, 200.0));
    assert_eq!(
        walled(across, display, banded),
        across.min,
        "an edge with no band, toward another display, holds nothing back"
    );
    let left = Rect::from_min_max(pos2(840.0, 0.0), pos2(1600.0, 600.0));
    assert_eq!(
        walled(across, display, left),
        pos2(840.0, 100.0),
        "a band down the left edge pushes the window off it"
    );
    let tall = Rect::from_min_size(pos2(1000.0, 100.0), vec2(300.0, 700.0));
    assert_eq!(
        walled(tall, display, banded),
        pos2(1000.0, 0.0),
        "a window taller than the room keeps its top on the display"
    );
}
