use super::{options, run_ok, scratch, shell};

#[test]
fn passed_is_written_only_when_the_test_passes() {
    let directory = scratch("passed");
    let passed = directory.join("passed");
    let mut failing = options(shell("exit 3"));
    failing.passed = Some(passed.clone());
    assert_eq!(run_ok(&failing), 3);
    assert!(!passed.exists());
    let mut passing = options(shell("exit 0"));
    passing.passed = Some(passed.clone());
    assert_eq!(run_ok(&passing), 0);
    assert!(passed.is_file());
}
