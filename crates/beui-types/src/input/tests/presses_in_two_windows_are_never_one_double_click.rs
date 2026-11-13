use super::*;

use crate::window::WindowId;

#[test]
fn presses_in_two_windows_are_never_one_double_click() {
    let mut counter = ClickCounter::default();
    let now = Instant::now();
    let at = Pos2::new(40.0, 30.0);
    let (outer, inner) = (WindowId::next(), WindowId::next());
    assert_eq!(counter.count(at, outer, now), 1);
    assert_eq!(
        counter.count(at, inner, now),
        1,
        "the same press, handed on to another window, counted as a second click"
    );
    assert_eq!(
        counter.count(at, inner, now),
        2,
        "a second press in the same window is a double-click"
    );
}
