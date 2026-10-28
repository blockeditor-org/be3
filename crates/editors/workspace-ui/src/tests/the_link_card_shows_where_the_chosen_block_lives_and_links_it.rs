use block_editor_beui::{BlockLocation, PickedBlock};

use super::*;

#[test]
fn the_link_card_shows_where_the_chosen_block_lives_and_links_it() {
    let (mut fixture, _) = editor_sized(Some(Vec2::new(1000.0, 600.0)));
    fixture.test.catalog(catalog());
    let places = places(&mut fixture);
    let (notes, other) = (Uuid::new_v4(), Uuid::new_v4());
    show(&mut fixture, other, None);
    show(&mut fixture, notes, None);
    let mut fixture = picking(&fixture, filter(), BlockLocation::Block(notes.into_bytes()));
    fixture.settle();
    fixture.test.click("picker.link-existing");
    fixture
        .test
        .click(format!("picker.link.{}.chevron", places.work));
    fixture
        .test
        .click(format!("picker.link.{}.row", places.minutes));
    fixture
        .test
        .snapshot("the_link_card_shows_where_the_chosen_block_lives");

    fixture.test.click("picker.confirm");

    assert_eq!(
        fixture.test.take_picked(),
        vec![Ok(Some(PickedBlock {
            block_id: places.minutes.into_bytes(),
            block_type: SHOWN_TYPE.into_bytes(),
            linked: true,
            placed: false,
        }))],
        "the block chosen is the one linked"
    );
}
