use super::*;

#[test]
fn super_d_hides_every_window_and_brings_them_back() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.host.show_block(be_sdk_beui::ShowRequest {
        block_id: Uuid::new_v4(),
        block_type: SHOWN_TYPE,
        via: None,
        editor: None,
    });
    fixture.settle();
    fixture.test.clear_calls();

    assert!(
        fixture.test.host_key(Modifiers::LOGO, Key::D),
        "the desktop takes Super+D before the program that has the keyboard"
    );
    fixture.settle();
    let mut hidden = fixture.test.take_host_calls(be_sdk_test::requested::window);
    hidden.sort_by_key(|action| format!("{action:?}"));
    assert_eq!(
        hidden,
        [
            WindowAction::Hide(HostWindowId(1)),
            WindowAction::Hide(HostWindowId(2))
        ],
        "Super+D hides every program window"
    );
    fixture.show_windows(&[
        HostWindow {
            shown: false,
            ..host_window(1, false)
        },
        HostWindow {
            shown: false,
            ..host_window(2, false)
        },
    ]);
    assert!(
        fixture.window_rect(1).is_none() && fixture.window_rect(2).is_none(),
        "and the shell draws none of them"
    );
    assert!(
        fixture.placed_blocks().is_empty(),
        "nor the block window: {:?}",
        fixture.placed_blocks()
    );

    assert!(fixture.test.host_key(Modifiers::LOGO, Key::D));
    fixture.settle();
    let shown: Vec<WindowAction> = fixture
        .test
        .take_host_calls(be_sdk_test::requested::window)
        .into_iter()
        .filter(|action| matches!(action, WindowAction::Show(_)))
        .collect();
    assert_eq!(
        shown.len(),
        2,
        "a second Super+D shows them all again: {shown:?}"
    );
    assert!(
        !fixture.placed_blocks().is_empty(),
        "the block window too"
    );
}
