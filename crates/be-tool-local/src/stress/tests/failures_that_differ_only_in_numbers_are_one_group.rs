use super::*;

#[test]
fn failures_that_differ_only_in_numbers_are_one_group() {
    assert_eq!(
        normalise("the painting has 258 shapes, it used to have 259"),
        normalise("the painting has 259 shapes, it used to have 258")
    );
    let failure = |reason: &str, log: &str| Failure {
        reason: reason.to_owned(),
        log: PathBuf::from(log),
    };
    let failures = [
        failure("timed out after 30 seconds", "1.log"),
        failure("timed out after 31 seconds", "2.log"),
        failure("timed out after 30 seconds", "3.log"),
        failure("timed out after 30 seconds", "4.log"),
        failure("something else", "5.log"),
    ];
    let report = report(&failures);
    assert!(
        report.contains("4 x timed out after N seconds (1.log, 2.log, 3.log, ...)"),
        "{report}"
    );
    assert!(report.contains("1 x something else (5.log)"), "{report}");
    assert_eq!(file_name("a b/c"), "a_b_c");
}
