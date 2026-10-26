use block_editor_beui::BlockLocation;

use super::*;

#[test]
fn a_pick_withdrawn_by_the_editor_that_asked_closes_the_picker() {
    let (mut fixture, _) = editor();
    fixture
        .test
        .request_pick(7, filter(), BlockLocation::Root, None);
    fixture.settle();
    assert!(fixture.test.shown("picker.link-existing"));

    fixture.test.withdraw_pick(7);
    fixture.settle();

    assert!(
        !fixture.test.shown("picker.link-existing"),
        "the picker closes once the pick is withdrawn"
    );
    assert_eq!(
        fixture.test.take_pick_answers(),
        vec![(7, Ok(None))],
        "and the withdrawn call ends as cancelled"
    );
}
