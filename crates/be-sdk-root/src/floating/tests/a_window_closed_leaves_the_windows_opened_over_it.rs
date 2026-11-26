use super::*;

#[test]
fn a_window_closed_leaves_the_windows_opened_over_it() {
    // A window opened from another opens over it, as a window of its own; closing the first
    // closes only it, as be-sdk-window-stack closes one under be-host: its app closes the
    // others it opened over it itself.
    let mut windows = Vec::new();
    let WindowRequest::Open(mut over) = open(8, 2, pos2(10.0, 10.0)) else {
        unreachable!("`open` opens");
    };
    over.opener = ViewRef {
        instance: EDITOR,
        view: ViewId(7),
    };
    over.placement = WindowPlacement::Over(WindowOver {
        view: over.opener,
        at: pos2(10.0, 10.0),
    });
    let _ = hear(
        &mut windows,
        &[main_screen(true)],
        vec![open(7, 1, pos2(40.0, 30.0)), WindowRequest::Open(over)],
        None,
        Pos2::ZERO,
    );
    let rects = |windows: &[Floating]| {
        windows
            .iter()
            .map(|window| (window.view, window.rect.min))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        rects(&windows),
        [(ViewId(7), pos2(40.0, 30.0)), (ViewId(8), pos2(50.0, 40.0))],
        "the second opens over the first, where it says in the first's points"
    );
    let _ = hear(
        &mut windows,
        &[main_screen(true)],
        vec![WindowRequest::Close(ViewId(7))],
        None,
        Pos2::ZERO,
    );
    assert_eq!(rects(&windows), [(ViewId(8), pos2(50.0, 40.0))]);
}
