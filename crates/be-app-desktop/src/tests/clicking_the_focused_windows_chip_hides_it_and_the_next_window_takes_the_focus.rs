use super::*;

#[test]
fn clicking_the_focused_windows_chip_hides_it_and_the_next_window_takes_the_focus() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.show_windows(&[
        foot(1),
        TestWindow {
            focused: true,
            ..notes(2)
        },
    ]);
    fixture.show_windows(&[
        TestWindow {
            focused: true,
            ..foot(1)
        },
        notes(2),
    ]);
    fixture.focused_windows();

    fixture.test.click("desktop.windows.1");
    fixture.settle();
    assert!(
        fixture.window_rect(1).is_none(),
        "the focused window's chip hides it"
    );
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(2)],
        "and the window behind it takes the focus"
    );

    let block = Uuid::new_v4();
    fixture.host.show_block(be_sdk_beui::ShowRequest {
        block_id: block,
        block_type: SHOWN_TYPE,
        via: None,
        editor: None,
    });
    fixture.settle();
    let chip = fixture.block_chip();
    let chip = chip.as_str();
    assert_eq!(
        fixture.placed_blocks().len(),
        1,
        "a block shown on the desktop opens in a window with the focus"
    );

    fixture.test.click(chip);
    fixture.settle();
    assert!(
        fixture.placed_blocks().is_empty(),
        "a click on the focused block window's chip hides the window"
    );
    assert!(fixture.test.shown(chip), "and keeps its chip");

    fixture.test.click(chip);
    fixture.settle();
    assert_eq!(
        fixture.placed_blocks().len(),
        1,
        "a click on a hidden window's chip shows it again"
    );
    fixture.test.click(chip);
    fixture.settle();
    assert!(
        fixture.placed_blocks().is_empty(),
        "and it took the focus, so the next click hides it again"
    );
}
