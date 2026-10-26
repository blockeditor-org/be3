use block_editor_beui::BlockLocation;
use block_editor_beui::beui::Modifiers;

use super::*;

#[test]
fn searching_the_link_tree_keeps_matches_and_the_folders_above_them() {
    let (mut fixture, _) = editor();
    let places = places(&mut fixture);
    fixture
        .test
        .request_pick(9, filter(), BlockLocation::Root, None);
    fixture.settle();
    fixture.test.click("picker.link-existing");
    fixture.settle();
    assert!(
        !fixture
            .test
            .shown(&format!("picker.link.{}.row", places.minutes)),
        "a folder starts closed"
    );

    fixture.test.click("picker.search");
    fixture.test.type_text("minu");
    fixture.settle();
    assert!(
        !fixture
            .test
            .shown(&format!("picker.link.{}.row", places.minutes)),
        "search reaches only folders that have been opened"
    );

    for _ in 0.."minu".len() {
        fixture
            .test
            .key(block_editor_beui::beui::Key::Backspace, Modifiers::NONE);
    }
    fixture.settle();
    fixture
        .test
        .click(format!("picker.link.{}.chevron", places.work));
    fixture.settle();
    fixture.test.click("picker.search");
    fixture.test.type_text("minu");
    fixture.settle();
    for kept in [places.space, places.work, places.minutes] {
        assert!(
            fixture.test.shown(&format!("picker.link.{kept}.row")),
            "a match and the folders above it are shown, opened"
        );
    }
    for hidden in [places.trips, places.q4, places.budget] {
        assert!(
            !fixture.test.shown(&format!("picker.link.{hidden}.row")),
            "what does not match is hidden"
        );
    }

    for _ in 0.."minu".len() {
        fixture
            .test
            .key(block_editor_beui::beui::Key::Backspace, Modifiers::NONE);
    }
    fixture.test.type_text(&places.budget.to_string());
    fixture.settle();
    assert!(
        fixture
            .test
            .shown(&format!("picker.link.{}.row", places.budget)),
        "a block is found by its UUID too"
    );
    assert!(
        !fixture
            .test
            .shown(&format!("picker.link.{}.row", places.minutes)),
        "and only that block"
    );
}
