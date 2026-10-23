use super::*;

#[test]
fn the_terminal_switches_to_its_setup_when_the_host_leaves() {
    let mut harness = Harness::new(true);
    harness.connect_host();
    harness.snapshot_of("$ ");
    assert!(
        harness
            .editor
            .document()
            .find_test_id("terminal.offline")
            .is_none()
    );

    harness.disconnect_host();
    assert!(harness.shown("terminal.offline").contains("No machine"));

    harness.sent();
    harness.connect_host();
    assert_eq!(
        attaches(&harness.sent()),
        1,
        "a host that comes back is asked for its screen"
    );
    assert_eq!(harness.shown("terminal.status"), "\"Connecting…\"");
}
