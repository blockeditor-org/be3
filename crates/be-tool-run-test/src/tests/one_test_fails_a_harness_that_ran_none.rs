use super::{options, run_ok, scratch, shell};

#[test]
fn one_test_fails_a_harness_that_ran_none() {
    let mut ran_one = options(shell("echo 'running 1 test'; echo ok"));
    ran_one.one_test = true;
    assert_eq!(run_ok(&ran_one), 0);
    let mut ran_none = options(shell("echo 'running 0 tests'"));
    ran_none.one_test = true;
    assert_eq!(run_ok(&ran_none), 1);
    let mut failed = options(shell("echo 'running 1 test'; exit 101"));
    failed.one_test = true;
    assert_eq!(run_ok(&failed), 101);

    let directory = scratch("one-test");
    let mut recorded = options(shell("echo 'running 0 tests'"));
    recorded.one_test = true;
    recorded.record = Some(directory.join("none"));
    assert_eq!(run_ok(&recorded), 0);
    assert_eq!(
        std::fs::read_to_string(directory.join("none/status")).unwrap(),
        "1\n"
    );
}
