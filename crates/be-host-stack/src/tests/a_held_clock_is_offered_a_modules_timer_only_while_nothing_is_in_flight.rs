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
    // The stack holds the clock at a frame's instant; on the harness's manual clock that instant
    // reads as the clock's own time (`Clock::at`), which nothing in this test moves.
    let held = harness.clock.now();
    let anchor = std::time::Instant::now();
    hold_clock(Some(anchor));

    add(owed, 2);
    wait_until("owes a seal for the edit", |shared| shared.owing > 0);
    let due = crate::wait_until(|shared| shared.modules_due);
    assert!(due > anchor, "the seal is due after the held moment");
    assert_eq!(harness.clock.now(), held, "the modules' clock stood still");

    harness.relay().hold_requests(true);
    add(sent, 3);
    wait_until("owes a seal for both edits", |shared| shared.owing == 2);
    // Sealed, `sent`'s edit is published and waits on the stalled server; `owed` edits again,
    // so a module timer waits too.
    seal_owed();
    wait_until("the seal waits on the server", |shared| shared.waiting > 0);
    add(owed, 4);
    // `parked` is set while the worker sleeps between rounds, so the round that saw the new edit
    // has ended and published what it offers.
    let due = crate::wait_until(|shared| {
        (shared.owing > 0 && shared.parked.is_some()).then_some(shared.modules_due)
    });
    assert_eq!(
        due, None,
        "with a publish in flight, the server's answer can make progress, so no timer is offered"
    );

    // Sealing what was owed then settles nothing later: once the server answers, the edit made
    // after the seal is still owed, for the module's timer.
    harness.relay().hold_requests(false);
    wait_until("the publish is answered and the worker rests", |shared| {
        shared.waiting == 0 && !shared.draining && shared.parked.is_some()
    });
    assert!(
        crate::wait_until(|shared| Some(shared.owing)) > 0,
        "the edit made after the seal is still owed"
    );
}
