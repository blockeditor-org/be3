use super::*;

#[test]
fn a_window_keeps_its_bar_where_it_can_be_grabbed() {
    let display = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let works = [display];
    let size = vec2(300.0, 200.0);
    let at = |x: f32, y: f32| grabbable(Rect::from_min_size(pos2(x, y), size), &works, display);
    assert_eq!(
        at(100.0, -150.0),
        pos2(100.0, 0.0),
        "a bar pushed above the display comes back down onto it"
    );
    assert_eq!(
        at(100.0, 590.0),
        pos2(100.0, 600.0 - KEPT.y),
        "a bar pushed below the display comes back up until all of its height shows"
    );
    assert_eq!(
        at(780.0, 100.0),
        pos2(800.0 - KEPT.x, 100.0),
        "a window pushed off the right keeps enough of its bar to grab"
    );
    assert_eq!(
        at(-280.0, 100.0),
        pos2(KEPT.x - size.x, 100.0),
        "and so does one pushed off the left"
    );
    assert_eq!(
        at(-150.0, 300.0),
        pos2(-150.0, 300.0),
        "a window whose bar can be grabbed stays where it is, however much of it is off"
    );

    let below = Rect::from_min_size(pos2(0.0, 600.0), vec2(800.0, 600.0));
    let both = [display, below];
    let tall = Rect::from_min_size(pos2(100.0, 500.0), vec2(300.0, 600.0));
    assert_eq!(
        grabbable(tall, &both, below),
        tall.min,
        "a bar on another display than the one holding most of the window is still reachable"
    );
    let beside = Rect::from_min_size(pos2(800.0, 0.0), vec2(800.0, 600.0));
    let across = Rect::from_min_size(pos2(700.0, 100.0), size);
    assert_eq!(
        grabbable(across, &[display, beside], beside),
        across.min,
        "a bar across two displays side by side is reachable"
    );
}
