use super::*;

#[test]
fn the_terminal_keeps_its_scrollback_and_says_so_when_the_host_leaves() {
    let mut harness = Harness::new(true);
    let history: Vec<TerminalLine> = (0..30).map(log_line).collect();
    harness.hold(Screen::rows(vec![command("ls"), listing(), prompt()]).history(history));
    harness.connect_host();
    assert!(
        harness
            .editor
            .document()
            .find_test_id("terminal.offline")
            .is_none()
    );

    harness.disconnect_host();
    assert!(harness.shown("terminal.offline").contains("No machine"));
    assert!(harness.says("crate number 29"), "the scrollback stays");
    assert!(harness.says("README.md"), "and so does the last screen");
    harness.editor.snapshot("a_terminal_without_its_machine");

    harness.editor.click("terminal.setup");
    harness.editor.settle();
    let command = harness.shown("terminal.command");
    assert!(command.contains("terminal-host"), "{command}");
    harness
        .editor
        .snapshot("a_terminal_without_its_machine_shows_how_to_start_one");
    harness.editor.key(Key::Escape, Modifiers::NONE);
    harness.editor.settle();

    harness.sent();
    harness.connect_host();
    assert!(
        resizes(&harness.sent()) >= 1,
        "a host that comes back is told the size again"
    );
    assert_eq!(harness.shown("terminal.status"), "\"\"");
}
