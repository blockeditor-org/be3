use block_editor_beui::BlockCommand;
use block_editor_beui::beui::Vec2;

use super::*;

#[test]
fn the_phone_files_header_opens_the_app_menu() {
    let mut fixture = editor_sized(Some(Vec2::new(390.0, 800.0)));
    fixture.settle();

    fixture.test.click("file-tree.app-menu");
    fixture.settle();

    assert!(
        fixture
            .test
            .take_block_commands()
            .iter()
            .any(|(_, command)| *command == BlockCommand::AppMenu),
        "the account button asks the host for its menu"
    );
}
