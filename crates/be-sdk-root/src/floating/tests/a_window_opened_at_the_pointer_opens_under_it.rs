use super::*;

#[test]
fn a_window_opened_at_the_pointer_opens_under_it() {
    // Its corner a little above and left of the pointer, so the pointer is over its grip, as
    // be-host's desktop opens one.
    let mut windows = Vec::new();
    let WindowRequest::Open(mut opened) = open(7, 1, Pos2::ZERO) else {
        unreachable!("`open` opens");
    };
    opened.placement = WindowPlacement::AtPointer;
    let _ = hear(
        &mut windows,
        &[main_screen(true)],
        vec![WindowRequest::Open(opened)],
        None,
        pos2(300.0, 200.0),
    );
    assert_eq!(
        windows.iter().map(|window| window.rect.min).collect::<Vec<_>>(),
        [pos2(300.0, 200.0) - be_sdk_window_stack::OPEN_GRAB]
    );
}
