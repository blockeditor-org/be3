use super::*;

#[test]
fn a_window_shown_in_the_app_is_kept_inside_it_and_shrinks_rather_than_covering_its_opener() {
    let viewport = vec2(800.0, 600.0);
    let fits = Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 200.0));
    assert_eq!(
        clamped(fits, viewport),
        fits,
        "a window that fits stays where it was asked"
    );

    let wide = Rect::from_min_size(pos2(600.0, 100.0), vec2(300.0, 200.0));
    assert_eq!(
        clamped(wide, viewport),
        Rect::from_min_size(pos2(500.0, 100.0), vec2(300.0, 200.0)),
        "one past the right edge moves back in"
    );

    let tall = Rect::from_min_size(pos2(100.0, 300.0), vec2(300.0, 500.0));
    assert_eq!(
        clamped(tall, viewport),
        Rect::from_min_size(pos2(100.0, 300.0), vec2(300.0, 300.0)),
        "one past the bottom keeps its top and shrinks to the room below it"
    );

    let low = Rect::from_min_size(pos2(100.0, 520.0), vec2(300.0, 200.0));
    assert_eq!(
        clamped(low, viewport),
        Rect::from_min_size(pos2(100.0, 400.0), vec2(300.0, 200.0)),
        "with too little room below, it moves up instead"
    );

    let huge = Rect::from_min_size(pos2(-50.0, -20.0), vec2(1000.0, 900.0));
    assert_eq!(
        clamped(huge, viewport),
        Rect::from_min_size(Pos2::ZERO, viewport),
        "one larger than the app fills it"
    );
}
