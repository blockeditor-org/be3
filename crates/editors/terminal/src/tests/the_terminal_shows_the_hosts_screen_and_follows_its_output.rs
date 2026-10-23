use super::*;

#[test]
fn the_terminal_shows_the_hosts_screen_and_follows_its_output() {
    let mut harness = Harness::new(true);

    harness.connect_host();
    assert_eq!(attaches(&harness.sent()), 1, "it asks the host for the screen");
    assert_eq!(harness.shown("terminal.status"), "\"Connecting…\"");

    harness.snapshot_of("\u{1b}[1;32mada@build-box\u{1b}[0m:~$ ls\r\nREADME.md  \u{1b}[1;34msrc\u{1b}[0m\r\n\u{1b}[1;32mada@build-box\u{1b}[0m:~$ ");
    harness.host_says(TerminalMessage::Output {
        shell: 1,
        sequence: 5,
        bytes: b"claude".to_vec(),
    });
    harness.host_says(TerminalMessage::Output {
        shell: 1,
        sequence: 5,
        bytes: b" twice".to_vec(),
    });

    assert_eq!(harness.shown("terminal.host"), "\"ada on build-box\"");
    assert_eq!(harness.shown("terminal.status"), "\"\"");
    assert!(harness.sent().is_empty(), "the output followed on");
    harness
        .editor
        .snapshot("the_terminal_shows_the_hosts_screen_and_follows_its_output");
}
