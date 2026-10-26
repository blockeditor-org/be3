use super::*;

#[test]
fn the_terminal_says_its_machine_is_offline_and_how_to_start_one() {
    let mut harness = Harness::new(true);

    assert!(harness.shown("terminal.offline").contains("No machine"));
    let command = harness.shown("terminal.command");
    assert!(
        command.contains(&format!("be {} terminal-host", harness.block)),
        "{command}"
    );
    assert_eq!(
        setup_command(harness.block),
        format!(
            "BE3_AGENT_CREDENTIALS='<the agent's credentials>' be {} terminal-host",
            harness.block
        )
    );
    assert!(harness.sent().is_empty(), "nobody is there to ask");
    harness
        .editor
        .snapshot("the_terminal_says_its_machine_is_offline_and_how_to_start_one");
}
