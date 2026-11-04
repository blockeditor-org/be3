use std::time::Duration;

use super::worker::reconnect_delay;

#[test]
fn backing_off_waits_longer_after_each_failure_up_to_a_ceiling() {
    let longest: Vec<Duration> = (0..8)
        .map(|failures| reconnect_delay(failures, 1.0))
        .collect();
    assert_eq!(
        longest,
        [500, 1000, 2000, 4000, 8000, 16000, 30000, 30000].map(Duration::from_millis)
    );
    assert_eq!(reconnect_delay(3, 0.0), Duration::from_millis(2000));
    assert_eq!(reconnect_delay(u32::MAX, 0.5), Duration::from_millis(22500));
}
