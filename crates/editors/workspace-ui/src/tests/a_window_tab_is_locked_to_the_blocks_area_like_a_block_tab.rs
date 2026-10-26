use super::*;

fn window_in_sidebar(fixture: &Fixture, id: u64) -> Option<bool> {
    placed_windows(fixture)
        .into_iter()
        .find(|(window, _)| *window == id)
        .map(|(_, rect)| rect.right() <= SIDEBAR_EDGE + 1.0)
}

#[test]
fn a_window_tab_is_locked_to_the_blocks_area_like_a_block_tab() {
    let (mut fixture, _) = editor_sized(Some(WIDE));
    fixture.host.set_watched::<WindowsWatch>(&vec![
        window(7, "Terminal", None),
        window(8, "Browser", None),
    ]);
    fixture.settle();

    fixture.drag_text("Terminal", SIDEBAR);
    fixture.click_text("Terminal");
    assert_eq!(
        window_in_sidebar(&fixture, 7),
        Some(false),
        "a program window's tab stays in the blocks area"
    );

    open_tab_menu(&mut fixture, "Terminal");
    fixture.click_text("Unlock");
    fixture.drag_text("Terminal", BELOW_SIDEBAR);
    assert_eq!(
        window_in_sidebar(&fixture, 7),
        Some(true),
        "once unlocked it can move into the sidebar"
    );
}
