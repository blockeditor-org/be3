use std::time::Duration;

use crate::peer::backoff;

/// The first write waits for nothing; after each refusal the wait is a step longer, and where in
/// its step it falls is the jitter's.
#[test]
fn an_update_waits_longer_after_each_refusal_within_its_bounds() {
    assert_eq!(backoff(0, u128::MAX), Duration::ZERO);
    for refused in 1..crate::UPDATE_ATTEMPTS {
        let step = Duration::from_millis(10 * refused as u64);
        assert_eq!(backoff(refused, 0), step);
        assert_eq!(backoff(refused, 10 * refused as u128), step * 2);
        for jitter in [1, 7, 12_345, u128::MAX] {
            let wait = backoff(refused, jitter);
            assert!(step <= wait && wait <= step * 2, "{wait:?} after {refused}");
        }
    }
}
