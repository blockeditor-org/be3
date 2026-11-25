use super::*;

use crate::modules::RESTARTS;

/// A type module that stops `RESTARTS` times within a minute is not started again, and the
/// host is told why, for the person, rather than the module going quiet.
#[test]
fn a_module_that_keeps_stopping_stops_for_good_and_says_so() {
    with_test_types_on(
        be_block_client::Clock::manual(),
        |_, _, mut replicas| async move {
            for _ in 1..RESTARTS {
                replicas.modules.restart_type(Tally::CONTENT_TYPE);
                assert_eq!(
                    replicas.take_stopped_modules(),
                    Vec::<String>::new(),
                    "it is started again"
                );
            }
            replicas.modules.restart_type(Tally::CONTENT_TYPE);
            let stopped = replicas.take_stopped_modules();
            assert_eq!(stopped.len(), 1, "{stopped:?}");
            assert!(stopped[0].contains("is not started again"), "{stopped:?}");
        },
    );
}
