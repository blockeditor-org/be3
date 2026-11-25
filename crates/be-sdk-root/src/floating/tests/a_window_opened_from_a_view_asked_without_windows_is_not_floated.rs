use super::*;

#[test]
fn a_window_opened_from_a_view_asked_without_windows_is_not_floated() {
    // The view draws the windows it opens itself: the root floats none of them.
    let mut windows = Vec::new();
    let changed = hear(
        &mut windows,
        &[main_screen(false)],
        vec![open(7, 1, pos2(40.0, 30.0))],
        None,
        Pos2::ZERO,
    )
    .0;
    assert!(!changed);
    assert!(windows.is_empty());
}
