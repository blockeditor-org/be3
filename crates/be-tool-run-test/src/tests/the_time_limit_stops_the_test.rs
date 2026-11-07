use std::time::Duration;

use super::{options, run_ok, scratch, shell};
use crate::TIMED_OUT;

#[test]
fn the_time_limit_stops_the_test() {
    let directory = scratch("time-limit");
    let out = directory.join("out");
    // The shell waits on a child of its own: both must stop for the test to.
    let mut options = options(shell("sleep 600 & wait"));
    options.time_limit = Some(("0s".to_owned(), Duration::ZERO));
    options.record = Some(out.clone());
    assert_eq!(run_ok(&options), 0);
    assert_eq!(std::fs::read_to_string(out.join("status")).unwrap(), format!("{TIMED_OUT}\n"));
    let log = std::fs::read_to_string(out.join("log")).unwrap();
    assert!(log.contains("the test ran for over 0s and was stopped"), "{log}");
}
