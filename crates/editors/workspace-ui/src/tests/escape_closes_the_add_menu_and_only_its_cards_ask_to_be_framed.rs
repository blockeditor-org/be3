use block_editor_beui::BlockLocation;
use block_editor_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn escape_closes_the_add_menu_and_only_its_cards_ask_to_be_framed() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let mut fixture = picking(&fixture, filter(), BlockLocation::Root);
    fixture.test.settle();
    assert!(
        !fixture.test.take_cards().contains(&true),
        "the menu draws its own surface, so it asks for no card"
    );
    fixture.test.click("picker.more");
    fixture.test.settle();
    assert_eq!(
        fixture.test.take_cards(),
        vec![true],
        "the More card asks the editor that placed it to frame it"
    );
    fixture.test.click("picker.back");
    fixture.test.settle();
    assert_eq!(fixture.test.take_cards(), vec![false]);

    fixture.test.key(Key::Escape, Modifiers::default());
    fixture.test.settle();
    assert_eq!(
        fixture.test.take_picked(),
        vec![Ok(None)],
        "Escape closes the menu, answering no block"
    );
}
