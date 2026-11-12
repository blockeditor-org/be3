use super::*;

#[test]
fn a_window_keeps_its_grip_where_it_can_be_grabbed() {
    let display = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let works = [display];
    let size = vec2(300.0, 200.0);
    let at = |x: f32, y: f32| grabbable(Rect::from_min_size(pos2(x, y), size), &works, display);
    assert_eq!(
        at(100.0, -150.0),
        pos2(100.0, 0.0),
        "a grip pushed above the display comes back down onto it"
    );
    assert_eq!(
        at(100.0, 590.0),
        pos2(100.0, 600.0 - KEPT.y),
        "a grip pushed below the display comes back up until all of its height shows"
    );
    assert_eq!(
        at(780.0, 100.0),
        pos2(800.0 - KEPT.x, 100.0),
        "a window pushed off the right keeps enough of its grip to grab"
    );
    assert_eq!(
        at(-280.0, 100.0),
        pos2(0.0, 100.0),
        "one pushed off the left comes back until its grip, in its top-left corner, shows"
    );
    assert_eq!(
        at(600.0, 500.0),
        pos2(600.0, 500.0),
        "a window whose grip can be grabbed stays where it is, however much of it is off"
    );

    let below = Rect::from_min_size(pos2(0.0, 600.0), vec2(800.0, 600.0));
    let both = [display, below];
    let tall = Rect::from_min_size(pos2(100.0, 500.0), vec2(300.0, 600.0));
    assert_eq!(
        grabbable(tall, &both, below),
        tall.min,
        "a grip on another display than the one holding most of the window is still reachable"
    );
    let beside = Rect::from_min_size(pos2(800.0, 0.0), vec2(800.0, 600.0));
    let across = Rect::from_min_size(pos2(700.0, 100.0), size);
    assert_eq!(
        grabbable(across, &[display, beside], beside),
        across.min,
        "a grip across two displays side by side is reachable"
    );
}
