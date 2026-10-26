use block_editor_beui::ShellCommand;

use super::*;

#[test]
fn the_desktop_files_toolbar_opens_the_app_menu() {
    let mut fixture = editor();
    fixture.settle();

    fixture.test.click("file-tree.desktop-app-menu");
    fixture.settle();

    assert!(
        fixture
            .test
            .take_shell_commands()
            .iter()
            .any(|(_, command)| matches!(command, ShellCommand::AppMenu { anchor: Some(_) })),
        "the account button asks the host for its menu"
    );
}
