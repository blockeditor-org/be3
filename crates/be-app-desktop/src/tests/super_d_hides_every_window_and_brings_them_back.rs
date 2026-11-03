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

    assert!(
        fixture.test.host_key(Modifiers::LOGO, Key::D),
        "the desktop takes Super+D before the program that has the keyboard"
    );
    fixture.settle();
    assert!(
        fixture.window_rect(1).is_none() && fixture.window_rect(2).is_none(),
        "Super+D hides every program window"
    );
    assert!(
        fixture.placed_blocks().is_empty(),
        "nor the block window: {:?}",
        fixture.placed_blocks()
    );

    assert!(fixture.test.host_key(Modifiers::LOGO, Key::D));
    fixture.settle();
    assert!(
        fixture.window_rect(1).is_some() && fixture.window_rect(2).is_some(),
        "a second Super+D shows them all again"
    );
    assert!(!fixture.placed_blocks().is_empty(), "the block window too");
}
