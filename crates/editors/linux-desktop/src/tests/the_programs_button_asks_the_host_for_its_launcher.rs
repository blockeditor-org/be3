use block_editor_beui::BlockCommand;

use super::*;

#[test]
fn the_programs_button_asks_the_host_for_its_launcher() {
    let mut fixture = Fixture::new();
    fixture.settle();

    fixture.test.click("desktop.launcher");
    fixture.settle();
    let commands = fixture.test.take_block_commands();
    assert!(
        commands
            .iter()
            .any(|(_, command)| *command == BlockCommand::Launcher),
        "the programs button asks the host for its launcher"
    );
    assert!(
        !commands
            .iter()
            .any(|(_, command)| *command == BlockCommand::AppMenu),
        "and not for its menu"
    );
}
