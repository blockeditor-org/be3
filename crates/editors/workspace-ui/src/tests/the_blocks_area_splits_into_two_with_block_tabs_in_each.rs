use super::*;

#[test]
fn the_blocks_area_splits_into_two_with_block_tabs_in_each() {
    let (mut fixture, places) = two_blocks();
    assert_eq!(fixture.shown(), vec![places.budget]);

    fixture.drag_text("Budget", block_editor_beui::beui::Pos2::new(WIDE.x - 40.0, WIDE.y / 2.0));

    let mut shown = fixture.shown();
    shown.sort();
    let mut both = vec![places.minutes, places.budget];
    both.sort();
    assert_eq!(shown, both, "each half of the blocks area shows its own block");
    assert_eq!(placed_in_sidebar(&fixture, places.minutes), Some(false));
    assert_eq!(placed_in_sidebar(&fixture, places.budget), Some(false));
    assert!(
        fixture.text_rect("Budget").left() > fixture.text_rect("Minutes").right(),
        "the tabs sit one in each half"
    );
    fixture
        .test
        .snapshot("the_blocks_area_splits_into_two_with_block_tabs_in_each");
}
