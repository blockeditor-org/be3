use block_editor_beui::{BlockLocation, ChildRect};

use super::*;

#[test]
fn the_location_page_shows_the_folders_as_a_tree_with_the_chosen_one_checked() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let places = places(&mut fixture);
    let mut placing = filter();
    placing.place = Some(BlockLocation::Block(places.q4.into_bytes()));
    placing.anchor = Some(ChildRect {
        x: 180.0,
        y: 120.0,
        width: 24.0,
        height: 24.0,
    });
    fixture
        .test
        .request_pick(6, placing, BlockLocation::Detached, None);
    fixture.settle();

    fixture.test.click("picker.location");
    fixture.settle();

    for shown in [places.space, places.trips, places.work, places.q4] {
        assert!(
            fixture.test.shown(&format!("picker.place.{shown}")),
            "the folders down to the chosen one are open"
        );
    }
    assert!(
        !fixture
            .test
            .shown(&format!("picker.place.{}", places.minutes)),
        "a block that holds nothing is no place to make one"
    );
    fixture
        .test
        .snapshot("the_location_page_shows_the_folders_as_a_tree_with_the_chosen_one_checked");
}
