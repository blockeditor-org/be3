use super::*;

#[test]
fn a_held_clock_is_offered_a_modules_timer_only_while_nothing_is_in_flight() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let (sent, owed) = (Uuid::new_v4(), Uuid::new_v4());
    open(sent, CounterContent::CONTENT_TYPE);
    open(owed, CounterContent::CONTENT_TYPE);
    add(sent, 1);
    add(owed, 1);
    wait_for_count(sent, 1);
    wait_for_count(owed, 1);
    flush();
    hold_clock(Some(std::time::Instant::now()));

    add(owed, 2);
    wait_until("owes a seal for the edit", |shared| shared.owing > 0);
    crate::wait_until(|shared| shared.modules_due);

    harness.relay().stall();
    add(sent, 3);
    wait_until("owes a seal for both edits", |shared| shared.owing == 2);
    // Sealed, `sent`'s edit is published and waits on the stalled server; `owed` edits again,
    // so a module timer waits too.
    seal_owed();
    wait_until("the seal waits on the server", |shared| shared.waiting > 0);
    add(owed, 4);
    let due = crate::wait_until(|shared| {
        (shared.owing > 0 && shared.parked.is_some()).then_some(shared.modules_due)
    });
    assert_eq!(
        due, None,
        "with a publish in flight, the server's answer can make progress, so no timer is offered"
    );
}
