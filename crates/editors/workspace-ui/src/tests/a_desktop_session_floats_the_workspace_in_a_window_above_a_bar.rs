use block_editor_beui::BlockCommand;

use super::*;

#[test]
fn a_desktop_session_floats_the_workspace_in_a_window_above_a_bar() {
    let (mut fixture, _) = desktop(None);

    assert!(fixture.says("Files"), "the files tab is open");
    assert!(fixture.says("No file open"), "the empty pane is beside it");
    assert!(
        fixture.says("Workspace"),
        "the split view is one tab of a group"
    );
    let files = fixture
        .test
        .children()
        .first()
        .map(|placement| placement.rect)
        .expect("the files are placed");
    assert!(
        files.x > 0.0 && files.y > 0.0,
        "the files float in a window rather than filling the screen from its corner, at {files:?}"
    );

    let document = fixture.test.document();
    let bar = document
        .find_test_id("workspace.desktop-bar")
        .and_then(|bar| document.node_rect(bar))
        .expect("the desktop has a bar");
    let window = document
        .find_test_id("workspace.files")
        .and_then(|files| document.node_rect(files))
        .expect("the files are laid out");
    assert!(
        bar.min.y >= window.max.y,
        "the bar runs along the bottom of the screen"
    );
    let clock = document
        .find_test_id("workspace.clock")
        .and_then(|clock| text_under(document, clock))
        .expect("the bar shows the time");
    assert!(
        clock.len() == 5 && clock.as_bytes()[2] == b':',
        "the time reads as hours and minutes, not {clock:?}"
    );

    fixture.test.click("workspace.desktop-menu");
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
