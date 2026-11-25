use super::*;

#[test]
fn failures_spread_beyond_the_window_never_stop_it() {
    let mut budget = FailureBudget::new(2, Duration::from_secs(60));
    for minute in 0..10 {
        assert_eq!(
            budget.fail(Duration::from_secs(minute * 61)),
            Failed::Again,
            "a failure a minute apart from the last is the only one in its window"
        );
    }
}
