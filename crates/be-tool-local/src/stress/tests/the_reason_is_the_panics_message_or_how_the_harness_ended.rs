use super::*;

#[test]
fn the_reason_is_the_panics_message_or_how_the_harness_ended() {
    let text = "running 1 test\n\nthread 'tests::a' (4) panicked at src/lib.rs:296:9:\ntimed out after 30 seconds; the app is starting\nstack backtrace:\n";
    assert_eq!(
        reason(text, "exited"),
        "timed out after 30 seconds; the app is starting"
    );
    assert_eq!(
        reason("no panic in here\n", "the harness was killed by signal 9"),
        "the harness was killed by signal 9"
    );
}
