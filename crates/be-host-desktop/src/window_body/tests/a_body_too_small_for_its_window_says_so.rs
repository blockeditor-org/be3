use super::*;

#[test]
fn a_body_too_small_for_its_window_says_so() {
    assert!(!too_small(vec2(300.0, 200.0), vec2(300.0, 200.0)));
    assert!(too_small(vec2(300.0, 200.0), vec2(299.0, 400.0)));
    assert!(too_small(vec2(0.0, 500.0), vec2(800.0, 400.0)));

    let mut shown = shown(status(vec2(640.0, 400.0), vec2(800.0, 300.0), true));
    assert!(
        shown.has("host.window.too_small"),
        "the body says it is too small for the window"
    );
    shown
        .stepped
        .snapshot("a_body_too_small_for_its_window_says_so");

    shown
        .status
        .set(Some(status(vec2(640.0, 400.0), vec2(600.0, 300.0), true)));
    settle(&mut shown.stepped);
    assert!(
        !shown.has("host.window.too_small"),
        "with room for it, the notice goes"
    );
}
