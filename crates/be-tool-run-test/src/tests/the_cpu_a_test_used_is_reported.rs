use std::fs::read_to_string;

use super::{options, run_ok, scratch, shell};
use crate::{Options, cpu_seconds};

#[test]
fn the_cpu_a_test_used_is_reported() {
    let directory = scratch("usage");
    let options = Options {
        record: Some(directory.clone()),
        ..options(shell("i=0; while [ $i -lt 20000 ]; do i=$((i+1)); done"))
    };
    assert_eq!(run_ok(&options), 0);
    let log = read_to_string(directory.join("log")).unwrap();
    let line = log.lines().last().unwrap();
    let seconds = cpu_seconds(line).unwrap_or_else(|| panic!("no usage line in {log:?}"));
    assert!(seconds >= 0.0, "{line}");
    assert!(line.contains("MB resident"), "{line}");
    assert_eq!(cpu_seconds("running 1 test"), None);
}
