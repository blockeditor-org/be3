use super::*;

#[test]
fn a_block_tab_unlocked_from_its_grip_moves_into_the_sidebar() {
    let (mut fixture, places) = two_blocks();
    open_grip_menu(&mut fixture, "editors");
    assert!(
        fixture.says("Locked"),
        "the blocks area's grip offers to unlock it"
    );
    fixture
        .test
        .snapshot("a_locked_block_areas_grip_offers_to_unlock_it");
    fixture.click_text("Locked");

    fixture.drag_text("Minutes", BELOW_SIDEBAR);

    assert_eq!(
        placed_in_sidebar(&fixture, places.minutes),
        Some(true),
        "a tab of an unlocked area can be split off below the sidebar's panels"
    );
    fixture
        .test
        .snapshot("a_block_tab_unlocked_from_its_grip_moves_into_the_sidebar");
}
