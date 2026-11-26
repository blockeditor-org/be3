use super::*;

#[test]
fn the_last_allowed_failure_within_the_window_stops_it() {
    let mut budget = FailureBudget::new(3, Duration::from_secs(60));
    assert_eq!(budget.fail(Duration::from_secs(1)), Failed::Again);
    assert_eq!(budget.fail(Duration::from_secs(2)), Failed::Again);
    assert_eq!(budget.fail(Duration::from_secs(3)), Failed::Stop);

    budget.reset();
    assert_eq!(
        budget.fail(Duration::from_secs(4)),
        Failed::Again,
        "started afresh, it has its whole budget again"
    );
}
