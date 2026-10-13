use block_editor_beui::be_block::{Calendar, EditorViewContent};
use block_editor_beui::beui::Key;

use super::*;

#[test]
fn clicking_the_clock_opens_the_desktops_calendar_and_clicking_away_closes_it() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        fixture.placed_blocks().is_empty(),
        "the calendar starts shut"
    );

    fixture.test.click("desktop.clock");
    fixture.settle();
    let calendar = fixture
        .calendar()
        .expect("the clock opens a calendar block");
    let desktop = fixture
        .host
        .view_block()
        .expect("the desktop is its own view");
    let info = fixture
        .test
        .store()
        .block(calendar)
        .expect("the calendar block was created");
    assert_eq!(info.block_type, Calendar::CONTENT_TYPE);
    assert_eq!(
        info.parent,
        BlockParent::Block(desktop),
        "the desktop owns its calendar"
    );
    let remembered = fixture
        .test
        .content::<EditorViewContent>(None)
        .root()
        .state("desktop.calendar")
        .map(|state| state.refs.clone());
    assert_eq!(remembered, Some(vec![calendar]), "the desktop remembers it");
    fixture.test.settle();
    fixture.test.snapshot("the_clock_opens_a_calendar");

    fixture.test.click("desktop.clock");
    fixture.settle();
    assert_eq!(
        fixture.calendar(),
        None,
        "clicking the clock again closes it"
    );

    fixture.test.click("desktop.clock");
    fixture.settle();
    assert_eq!(
        fixture.calendar(),
        Some(calendar),
        "opening it again shows the same block"
    );
    fixture.test.key_press(Key::Escape);
    fixture.settle();
    assert_eq!(fixture.calendar(), None, "Escape closes it");

    fixture.test.click("desktop.clock");
    fixture.settle();
    assert_eq!(fixture.calendar(), Some(calendar));
    fixture
        .test
        .click_at(block_editor_beui::beui::pos2(16.0, 16.0));
    fixture.settle();
    assert_eq!(fixture.calendar(), None, "a click outside closes it");

    assert_eq!(
        fixture.test.store().created(),
        vec![calendar],
        "the calendar is created once"
    );
}
