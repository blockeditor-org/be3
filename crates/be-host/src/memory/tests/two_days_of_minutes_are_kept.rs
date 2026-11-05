use super::*;

#[test]
fn two_days_of_minutes_are_kept() {
    let mut history = History::default();
    let minutes = MINUTES as u64 + 10;
    for minute in 0..minutes {
        history.record(sample(minute * 60, minute, 0));
    }
    let view = history.view().expect("samples have been taken");
    assert_eq!(
        view.points.len(),
        MINUTES + 1,
        "two days of ended minutes, and the minute so far"
    );
    assert_eq!(
        view.points.first().map(|point| point.at),
        Some((minutes - 1 - MINUTES as u64) * 60),
        "the oldest minutes go first"
    );
    assert_eq!(
        view.points.last().map(|point| point.at),
        Some((minutes - 1) * 60)
    );
}
