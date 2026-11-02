use super::*;

#[test]
fn a_window_held_past_the_edge_of_the_app_stretches_and_springs_back_when_let_go() {
    let mut window = Carrying::open(pos2(100.0, 100.0), vec2(300.0, 200.0));
    assert_eq!(
        window.shown(),
        Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 200.0))
    );

    let held = pos2(100.0, 100.0) + HELD;
    window.press(held);
    for x in [60.0, 0.0, -100.0, -200.0] {
        window.drag_to(pos2(x, held.y));
    }
    let stretched = window.shown();
    assert!(
        stretched.min.x < 0.0 && stretched.min.x > -250.0,
        "pulled past the left edge, the window follows the pointer only part of the way: {stretched:?}"
    );
    assert_eq!(stretched.size(), vec2(300.0, 200.0), "and keeps its size");

    window.release(pos2(-200.0, held.y));
    let released = window.shown().min.x;
    window.stepped.advance(STEP);
    window.stepped.frame(Vec::new());
    let returning = window.shown().min.x;
    assert!(
        returning > released && returning < 0.0,
        "let go, it moves back a part of the way each frame: {released} -> {returning}"
    );
    let frames = window.frames_to_settle();
    assert!(
        frames > 5 && frames < 120,
        "the spring takes a visible but short time: {frames} frames"
    );
    assert_eq!(
        window.shown(),
        Rect::from_min_size(pos2(0.0, 100.0), vec2(300.0, 200.0)),
        "it comes to rest against the edge"
    );
    assert_eq!(window.placed().min, pos2(0.0, 100.0), "where it is kept");
}
