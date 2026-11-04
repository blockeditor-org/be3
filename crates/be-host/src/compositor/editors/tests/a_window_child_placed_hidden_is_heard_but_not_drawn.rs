use super::*;

#[test]
fn a_window_child_placed_hidden_is_heard_but_not_drawn() {
    let window = HostWindowId(3);
    let shown = window_kind(window, true, false);
    let hidden = window_kind(window, true, true);
    assert_eq!(shown, Kind::Window(window));
    assert_eq!(
        hidden,
        Kind::HiddenWindow(window),
        "a window the shell places hidden is not drawn, so its program is suspended"
    );
    assert_eq!(heard_window(shown), Some(window));
    assert_eq!(
        heard_window(hidden),
        Some(window),
        "but its status is still the program's, and it can still be closed"
    );
    assert_eq!(
        window_kind(window, false, false),
        Kind::Unavailable,
        "only the shell places program windows"
    );
}
