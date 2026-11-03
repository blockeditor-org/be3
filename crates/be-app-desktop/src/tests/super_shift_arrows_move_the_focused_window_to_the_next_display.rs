use be_sdk_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn super_shift_arrows_move_the_focused_window_to_the_next_display() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 640.0));
    let right = Rect::from_min_size(pos2(960.0, 160.0), vec2(640.0, 480.0));
    let mut fixture = Fixture::new();
    fixture.on_two_displays((left, 1.0), (right, 1.0));
    fixture.show_windows(&[host_window(1, true)]);
    let was = fixture.test.desk_windows()[0];
    assert!(left.contains_rect(was));

    assert!(
        fixture.test.host_key(Modifiers { shift: true, ..Modifiers::LOGO }, Key::ArrowRight),
        "the desktop takes Super+Shift+Right before the program that has the keyboard"
    );
    fixture.settle();
    let moved = fixture.test.desk_windows()[0];
    assert!(
        right.contains_rect(moved),
        "Super+Shift+Right moves the window onto the next display: {moved:?}"
    );
    assert_eq!(moved.size(), was.size(), "at the size it was");

    assert!(fixture.test.host_key(Modifiers::LOGO, Key::ArrowLeft));
    fixture.settle();
    let snapped = fixture.test.desk_windows()[0];
    assert!(
        right.contains(snapped.center()) && snapped.width() < 0.6 * right.width(),
        "snapping keeps to the display the window is on: {snapped:?}"
    );

    assert!(
        fixture
            .test
            .host_key(Modifiers { shift: true, ..Modifiers::LOGO }, Key::ArrowLeft)
    );
    fixture.settle();
    let back = fixture.test.desk_windows()[0];
    assert!(
        left.contains(back.center()) && (back.left() - left.left()).abs() < 16.0,
        "a snapped window moved to another display is snapped there too: {back:?}"
    );
    assert!(
        back.width() > snapped.width() && back.width() < 0.6 * left.width(),
        "to that display's half: {back:?}"
    );

    assert!(fixture.test.host_key(Modifiers::LOGO, Key::ArrowDown));
    fixture.settle();
    let restored = fixture.test.desk_windows()[0];
    assert!(
        left.contains_rect(restored) && restored.size() == was.size(),
        "and restored there, it is as it was before it was snapped: {restored:?}"
    );
}
