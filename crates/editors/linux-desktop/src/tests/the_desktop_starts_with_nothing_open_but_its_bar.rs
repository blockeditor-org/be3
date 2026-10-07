use block_editor_beui::BlockCommand;

use super::*;

#[test]
fn the_desktop_starts_with_nothing_open_but_its_bar() {
    let mut fixture = Fixture::new();
    fixture.settle();

    assert!(fixture.placed_blocks().is_empty(), "no window is open");
    let document = fixture.test.document();
    assert!(document.find_test_id("desktop.bar").is_some());
    let clock = document
        .find_test_id("desktop.clock")
        .and_then(|clock| text_under(document, clock))
        .expect("the bar shows the time");
    assert!(
        clock.len() == 5 && clock.as_bytes()[2] == b':',
        "the time reads as hours and minutes, not {clock:?}"
    );

    fixture.test.click("desktop.menu");
    fixture.settle();
    assert!(
        fixture
            .test
            .take_block_commands()
            .iter()
            .any(|(_, command)| *command == BlockCommand::AppMenu),
        "the menu button asks the host for its menu"
    );
}
