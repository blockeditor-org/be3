use std::time::Duration;

use super::*;

use crate::modules::FAILURES_BEFORE_STOPPING;

/// A type module that stops `FAILURES_BEFORE_STOPPING` times within a minute is not started
/// again, and the host is told why, for the person, rather than the module going quiet; one
/// that stops as often but more slowly keeps being started again.
#[test]
fn a_module_that_keeps_stopping_stops_for_good_and_says_so() {
    let clock = be_block_client::Clock::manual();
    let moved = clock.clone();
    with_test_types_on(clock, |_, _, mut replicas| async move {
        let restart = |replicas: &mut Replicas<Quiet>| {
            replicas.modules.restart_type(Tally::CONTENT_TYPE);
            replicas.take_stopped_modules()
        };
        for _ in 1..FAILURES_BEFORE_STOPPING {
            assert_eq!(restart(&mut replicas), Vec::<String>::new(), "it is started again");
        }
        // A minute on, the failures before have left the window.
        moved.advance(Duration::from_secs(61));
        for _ in 1..FAILURES_BEFORE_STOPPING {
            assert_eq!(
                restart(&mut replicas),
                Vec::<String>::new(),
                "failing as often, but over more than a minute, it is started again"
            );
        }
        let stopped = restart(&mut replicas);
        assert_eq!(stopped.len(), 1, "{stopped:?}");
        assert!(stopped[0].contains("is not started again"), "{stopped:?}");
    });
}
