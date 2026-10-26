use super::*;

#[test]
fn a_block_tab_unlocked_from_its_menu_moves_into_the_sidebar() {
    let (mut fixture, places) = two_blocks();
    open_tab_menu(&mut fixture, "Minutes");
    assert!(
        fixture.says("Unlock"),
        "a locked tab's menu offers to unlock it"
    );
    fixture
        .test
        .snapshot("a_locked_block_tabs_menu_offers_to_unlock_it");
    fixture.click_text("Unlock");

    fixture.drag_text("Minutes", BELOW_SIDEBAR);

    assert_eq!(
        placed_in_sidebar(&fixture, places.minutes),
        Some(true),
        "an unlocked tab can be split off below the sidebar's panels"
    );
    fixture
        .test
        .snapshot("a_block_tab_unlocked_from_its_menu_moves_into_the_sidebar");
}
