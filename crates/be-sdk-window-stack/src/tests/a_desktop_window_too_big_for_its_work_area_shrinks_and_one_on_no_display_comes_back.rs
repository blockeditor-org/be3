use super::*;

#[test]
fn a_desktop_window_too_big_for_its_work_area_shrinks_and_one_on_no_display_comes_back() {
    let display = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 720.0));
    let banded = Rect::from_min_max(pos2(0.0, 0.0), pos2(1100.0, 679.0));
    let tall = Rect::from_min_size(pos2(100.0, 210.0), vec2(800.0, 700.0));
    assert_eq!(
        fitted(tall, display, banded),
        Rect::from_min_size(pos2(100.0, 0.0), vec2(800.0, 679.0)),
        "a window taller than the room above the band is shrunk to it"
    );
    let wide = Rect::from_min_size(pos2(-50.0, 100.0), vec2(1300.0, 200.0));
    assert_eq!(
        fitted(wide, display, banded),
        wide,
        "an axis no band insets is left free"
    );
    let lost = Rect::from_min_size(pos2(1500.0, 300.0), vec2(400.0, 300.0));
    assert_eq!(
        fitted(lost, display, banded),
        Rect::from_min_size(pos2(700.0, 300.0), vec2(400.0, 300.0)),
        "a window on no display is brought wholly into the work area it is given"
    );
    let others = [
        display,
        Rect::from_min_size(pos2(0.0, 720.0), vec2(1100.0, 720.0)),
    ];
    assert_eq!(
        home(
            &others,
            Rect::from_min_size(pos2(10.0, 700.0), vec2(100.0, 100.0))
        ),
        Some(1)
    );
    assert_eq!(
        home(&others, lost),
        Some(0),
        "the nearest display takes a window on none"
    );
    assert_eq!(home(&[], lost), None);
}
