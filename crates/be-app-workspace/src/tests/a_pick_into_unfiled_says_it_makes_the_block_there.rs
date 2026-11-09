use be_sdk_beui::BlockLocation;

use super::*;

#[test]
fn a_pick_into_unfiled_says_it_makes_the_block_there() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let mut placing = filter();
    placing.place = Some(BlockLocation::Unfiled);
    let mut fixture = picking(&fixture, placing, BlockLocation::Unfiled);
    fixture.settle();
    fixture.test.settle();

    assert!(
        fixture.test.label("picker.location").contains("Unfiled"),
        "the menu says where the new block goes: {}",
        fixture.test.label("picker.location")
    );
    fixture
        .test
        .snapshot("a_pick_into_unfiled_says_it_makes_the_block_there");
}
