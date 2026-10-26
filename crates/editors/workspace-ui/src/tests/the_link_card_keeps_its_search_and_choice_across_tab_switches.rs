use block_editor_beui::{BlockLocation, PickedBlock};

use super::*;

#[test]
fn the_link_card_keeps_its_search_and_choice_across_tab_switches() {
    let (mut fixture, _) = editor_sized(Some(Vec2::new(1000.0, 600.0)));
    fixture.test.catalog(catalog());
    let places = places(&mut fixture);
    let (notes, other) = (Uuid::new_v4(), Uuid::new_v4());
    show(&mut fixture, other, None);
    show(&mut fixture, notes, None);
    fixture.test.request_pick(
        8,
        filter(),
        BlockLocation::Block(notes.into_bytes()),
        Some(notes),
    );
    fixture.settle();
    fixture.test.click("picker.link-existing");
    fixture.settle();
    fixture
        .test
        .click(format!("picker.link.{}.chevron", places.work));
    fixture.settle();
    fixture
        .test
        .click(format!("picker.link.{}.row", places.minutes));
    fixture.settle();
    fixture
        .test
        .snapshot("the_link_card_shows_where_the_chosen_block_lives");

    show(&mut fixture, other, None);
    assert!(
        !fixture.test.shown("picker.confirm"),
        "the card hides with the tab it hangs from"
    );
    show(&mut fixture, notes, None);
    fixture.test.click("picker.confirm");
    fixture.settle();

    assert_eq!(
        fixture.test.take_pick_answers(),
        vec![(
            8,
            Ok(Some(PickedBlock {
                block_id: places.minutes.into_bytes(),
                block_type: SHOWN_TYPE.into_bytes(),
                linked: true,
                placed: false,
            }))
        )],
        "the block chosen before the switch is the one linked"
    );
}
