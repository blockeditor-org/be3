use super::*;

#[test]
fn raising_a_window_puts_it_on_top() {
    // The last opened is on top, until the first is raised.
    let mut windows = Vec::new();
    hear(
        &mut windows,
        &[main_screen(true)],
        vec![open(7, 1, pos2(0.0, 0.0)), open(8, 2, pos2(20.0, 20.0))],
    );
    let views = |windows: &[Floating]| windows.iter().map(|window| window.view).collect::<Vec<_>>();
    assert_eq!(views(&windows), [ViewId(7), ViewId(8)]);
    assert!(hear(
        &mut windows,
        &[main_screen(true)],
        vec![WindowRequest::Raise(ViewId(7))],
    ));
    assert_eq!(views(&windows), [ViewId(8), ViewId(7)]);
}
