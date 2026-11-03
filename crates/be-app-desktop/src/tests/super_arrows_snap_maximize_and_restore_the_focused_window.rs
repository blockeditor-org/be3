use super::*;

fn snap(fixture: &mut Fixture, key: Key) {
    assert!(
        fixture.test.host_key(Modifiers::LOGO, key),
        "the desktop takes Super+{key:?} before the program that has the keyboard"
    );
    fixture.settle();
}

fn focus(fixture: &mut Fixture, id: u64) {
    let windows = [1, 2].map(|window| host_window(window, window == id));
    fixture.show_windows(&windows);
}

#[test]
fn super_arrows_snap_maximize_and_restore_the_focused_window() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    let floating = fixture.window_rect(2).expect("window 2 is drawn");
    let dock = fixture.test.rect_of("desktop.dock");

    snap(&mut fixture, Key::ArrowLeft);
    focus(&mut fixture, 1);
    snap(&mut fixture, Key::ArrowRight);
    let left = fixture.window_rect(2).expect("window 2 is drawn");
    let right = fixture.window_rect(1).expect("window 1 is drawn");
    assert!(
        left.left() - dock.left() < 16.0 && dock.right() - right.right() < 16.0,
        "the windows reach the dock's sides: {left:?} {right:?} in {dock:?}"
    );
    assert!(
        left.right() <= right.left() && right.left() - left.right() < 16.0,
        "and meet in the middle: {left:?} {right:?}"
    );
    assert!(
        (left.width() - right.width()).abs() < 2.0 && (left.height() - right.height()).abs() < 1.0,
        "each taking half: {left:?} {right:?}"
    );
    fixture.test.snapshot("two_windows_snapped_side_by_side");

    snap(&mut fixture, Key::ArrowUp);
    let maximized = fixture.window_rect(1).expect("window 1 is drawn");
    assert!(
        maximized.left() - dock.left() < 16.0 && maximized.width() > 1.9 * right.width(),
        "Super+Up fills the windows' area: {maximized:?}"
    );
    assert!(
        !fixture.test.host_key(Modifiers::LOGO, Key::ArrowUp),
        "a maximized window leaves Super+Up alone"
    );

    fixture.test.clear_calls();
    snap(&mut fixture, Key::ArrowDown);
    let restored = fixture.window_rect(1).expect("window 1 is drawn");
    assert!(
        restored.width() < 0.9 * right.width(),
        "Super+Down puts it back as it was before it was snapped: {restored:?}"
    );
    assert!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::window)
            .is_empty(),
        "without hiding it"
    );

    snap(&mut fixture, Key::ArrowDown);
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::window),
        [
            WindowAction::Hide(HostWindowId(1)),
            WindowAction::Focus(HostWindowId(2))
        ],
        "Super+Down on a window that is not snapped hides it and focuses the one used before"
    );

    focus(&mut fixture, 2);
    snap(&mut fixture, Key::ArrowRight);
    assert_eq!(
        fixture.window_rect(2),
        Some(floating),
        "Super+Right on a window snapped left puts it back where it was"
    );
}
