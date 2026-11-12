use super::*;

#[test]
fn a_desktop_window_moved_past_the_foot_of_its_display_hangs_off_it_and_springs_back_to_its_grip() {
    let work = Rect::from_min_max(Pos2::ZERO, pos2(HOST.x, HOST.y - 40.0));
    let mut window = Carrying::on_desktop(pos2(100.0, 100.0), vec2(300.0, 200.0), work);
    assert_eq!(
        window.shown(),
        Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 200.0))
    );

    let held = pos2(100.0, 100.0) + HELD;
    let hanging = pos2(100.0, 450.0);
    window.press(held);
    window.drag_to(held + vec2(0.0, 200.0));
    window.drag_to(hanging + HELD);
    window.release(hanging + HELD);
    window.frames_to_settle();
    assert_eq!(
        window.shown(),
        Rect::from_min_size(hanging, vec2(300.0, 200.0)),
        "a window let go with most of it past the foot of its display and over the band there \
         stays, as long as its grip can be reached"
    );

    let held = hanging + HELD;
    let rest = work.max.y - be_sdk_window_stack::KEPT.y;
    window.press(held);
    for y in [520.0, 580.0, 640.0, 700.0] {
        window.drag_to(pos2(held.x, y));
    }
    let stretched = window.shown();
    assert!(
        stretched.min.y > rest && stretched.min.y < 700.0 - HELD.y,
        "pulled further, the window follows the pointer only part of the way: {stretched:?}"
    );
    assert_eq!(stretched.size(), vec2(300.0, 200.0), "and keeps its size");

    window.release(pos2(held.x, 700.0));
    let released = window.shown().min.y;
    window.stepped.advance(STEP);
    window.stepped.frame(Vec::new());
    let returning = window.shown().min.y;
    assert!(
        returning < released && returning > rest,
        "let go, it springs back a part of the way each frame rather than jumping: \
         {released} -> {returning}"
    );
    let frames = window.frames_to_settle();
    assert!(
        frames > 5 && frames < 120,
        "the spring takes a visible but short time: {frames} frames"
    );
    assert_eq!(
        window.shown(),
        Rect::from_min_size(pos2(100.0, rest), vec2(300.0, 200.0)),
        "it comes to rest with its grip just above the band"
    );
    let placement = window.inside(|key| placement_of(key));
    assert_eq!(
        placement,
        Some(be_sdk_window_stack::placement(pos2(100.0, rest))),
        "where it is kept"
    );
}
