use super::*;

#[test]
fn alt_f4_closes_the_focused_window_its_focused_dialog_or_a_block_window() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    assert!(
        fixture.test.host_key(Modifiers::ALT, Key::F4),
        "the desktop takes Alt+F4 before the program that has the keyboard"
    );
    fixture.settle();
    assert_eq!(
        fixture.closed_windows(),
        [HostWindowId(2)],
        "Alt+F4 asks the program to close the window with the keyboard"
    );

    let dialog = TestWindow {
        parent: Some(HostWindowId(2)),
        title: "Save changes?".to_owned(),
        ..host_window(3, true)
    };
    fixture.show_windows(&[host_window(1, false), host_window(2, false), dialog]);
    fixture.test.clear_calls();
    assert!(fixture.test.host_key(Modifiers::ALT, Key::F4));
    fixture.settle();
    assert_eq!(
        fixture.closed_windows(),
        [HostWindowId(3)],
        "over a window's dialog it closes the dialog, not the window"
    );

    fixture.host.show_block(be_sdk_beui::ShowRequest {
        block_id: Uuid::new_v4(),
        block_type: SHOWN_TYPE,
        via: None,
        editor: None,
    });
    fixture.settle();
    let chip = fixture.block_chip();
    assert!(fixture.test.shown(&chip));
    fixture.test.clear_calls();
    fixture.test.key(Key::F4, Modifiers::ALT);
    fixture.settle();
    assert!(
        !fixture.test.shown(&chip),
        "a block window with the keyboard closes too"
    );
    assert!(
        fixture.closed_windows().is_empty(),
        "and no program window is asked to close"
    );
}
