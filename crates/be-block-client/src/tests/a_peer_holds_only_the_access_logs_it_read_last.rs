use std::sync::Arc;

use uuid::Uuid;

use crate::peer::{SERVED_LOGS, ServedLogs};

/// A run that reads more logs than it holds for reading again lets go of the one read longest
/// ago, so however many it reads, it holds at most `SERVED_LOGS`, and one it keeps reading stays.
#[test]
fn a_peer_holds_only_the_access_logs_it_read_last() {
    let mut served = ServedLogs::default();
    let kept = Uuid::from_u128(1);
    served.insert(kept, Arc::new(Vec::new()));
    let first = Uuid::from_u128(2);
    served.insert(first, Arc::new(Vec::new()));
    for log in 3..(3 + 4 * SERVED_LOGS as u128) {
        assert!(served.get(kept).is_some(), "a log read again stays");
        served.insert(Uuid::from_u128(log), Arc::new(Vec::new()));
        assert!(served.len() <= SERVED_LOGS);
    }
    assert_eq!(served.len(), SERVED_LOGS);
    assert!(served.get(first).is_none(), "the log read longest ago was let go");
}
