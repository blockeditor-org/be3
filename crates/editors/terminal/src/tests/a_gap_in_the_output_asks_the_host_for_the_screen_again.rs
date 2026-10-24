use super::*;

#[test]
fn a_gap_in_the_output_asks_the_host_for_the_screen_again() {
    let mut harness = Harness::new(true);
    harness.connect_host();
    harness.snapshot_of("$ ");
    harness.sent();

    harness.host_says(TerminalMessage::Output {
        shell: 1,
        sequence: 7,
        bytes: b"late".to_vec(),
    });
    harness.host_says(TerminalMessage::Output {
        shell: 1,
        sequence: 8,
        bytes: b"later".to_vec(),
    });
    assert_eq!(
        attaches(&harness.sent()),
        1,
        "it asks once, and waits for the screen"
    );

    harness.host_says(TerminalMessage::Snapshot {
        shell: 1,
        sequence: 8,
        size: TerminalSize { cols: 80, rows: 24 },
        screen: b"$ latelater".to_vec(),
    });
    harness.host_says(TerminalMessage::Output {
        shell: 1,
        sequence: 9,
        bytes: b"!".to_vec(),
    });
    assert!(harness.sent().is_empty());
}
