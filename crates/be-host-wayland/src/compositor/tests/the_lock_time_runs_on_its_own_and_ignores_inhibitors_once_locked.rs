use super::*;

use std::time::Duration;

const BLANK: Duration = Duration::from_secs(600);
const LOCK: Duration = Duration::from_secs(300);

#[test]
fn the_lock_time_runs_on_its_own_and_ignores_inhibitors_once_locked() {
    let mut harness = Harness::new();
    harness.compositor().set_blank_after(Some(BLANK));
    harness.compositor().set_lock_after(Some(LOCK));
    harness.settle();

    harness.context().advance_clock(LOCK);
    harness.frame(Vec::new());
    assert!(harness.compositor().lock_due(), "the lock time ran out");
    assert!(!harness.compositor().idle(), "before the screens go off");
    harness.frame(vec![Input::PointerMoved(pos2(5.0, 5.0))]);
    assert!(
        !harness.compositor().lock_due(),
        "input starts the count again"
    );

    let (window, _) = harness.open();
    let _inhibitor = harness.client.inhibit(&window.surface);
    harness.settle();
    harness.context().advance_clock(BLANK * 2);
    harness.frame(Vec::new());
    assert!(
        !harness.compositor().lock_due(),
        "a shown inhibitor holds the lock off"
    );
    assert!(!harness.compositor().idle());

    harness.compositor().set_locked(true);
    harness.frame(Vec::new());
    harness.context().advance_clock(BLANK);
    harness.frame(Vec::new());
    assert!(
        harness.compositor().idle(),
        "a window behind the lock screen does not keep the screens on"
    );
}
