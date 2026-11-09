use super::*;

use be_block_client::{ClientError, Clock};

use crate::version::{Due, Failure, Statuses};
use crate::worker::next_retry;

const BLOCK: Uuid = Uuid::from_u128(0xb10c);
const INPUTS: crate::version::Signature = (1, 1, 1, 1);

#[test]
fn a_transient_version_failure_is_retried_on_the_backoff_and_on_reconnecting() {
    let clock = Clock::manual();
    let mut statuses = Statuses::default();
    let timed_out = || Failure::client(ClientError::TimedOut("reading a commit"));

    assert_eq!(
        statuses.failed(BLOCK, INPUTS, timed_out()),
        None,
        "a transient failure is not shown at first"
    );
    assert_eq!(
        statuses.due(BLOCK, INPUTS, false),
        Due::Waiting { transient: true },
        "an unrelated wake leaves it to the backoff"
    );
    let (armed, failures) = next_retry(true, clock.now(), None, 0, 0.0);
    assert_eq!(armed, Some(clock.now() + Duration::from_millis(250)));

    clock.advance(Duration::from_millis(100));
    assert_eq!(
        next_retry(true, clock.now(), armed, failures, 0.0),
        (armed, failures),
        "a wake before the deadline keeps it where it was"
    );

    clock.advance(Duration::from_millis(150));
    assert_eq!(
        statuses.due(BLOCK, INPUTS, true),
        Due::Compute,
        "the deadline tries it again"
    );
    let (later, failures) = next_retry(true, clock.now(), armed, failures, 0.0);
    assert_eq!(
        later,
        Some(clock.now() + Duration::from_millis(500)),
        "and the next waits longer"
    );
    assert_eq!(statuses.failed(BLOCK, INPUTS, timed_out()), None);
    let shown = statuses.failed(BLOCK, INPUTS, timed_out());
    assert!(
        shown.is_some(),
        "a transient failure that keeps failing shows why"
    );

    statuses.reconnected();
    assert_eq!(
        statuses.due(BLOCK, INPUTS, false),
        Due::Compute,
        "reconnecting tries it again at once"
    );
    assert_eq!(
        next_retry(false, clock.now(), later, failures, 0.0),
        (None, 0),
        "with nothing transient awaiting, no deadline stays armed"
    );
}
