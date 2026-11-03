use super::*;

#[test]
fn a_hidden_window_is_told_it_is_suspended_until_shown() {
    let mut harness = Harness::new();
    let (_window, id) = harness.open();
    assert!(!harness.client.received.suspended);

    harness.compositor().windows().set_hidden(id, true);
    harness.settle();
    assert!(
        harness.client.received.suspended,
        "a hidden window is suspended, so its program can stop drawing"
    );
    assert!(harness.compositor().windows().hidden(id));

    harness.compositor().windows().set_hidden(id, false);
    harness.settle();
    assert!(
        !harness.client.received.suspended,
        "shown again, it is no longer suspended"
    );
}
