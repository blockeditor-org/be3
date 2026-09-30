use super::*;

#[test]
fn the_week_starts_on_the_weekday_it_is_asked_for() {
    let saturday = Date::new(2026, 9, 26);
    assert_eq!(saturday.weekday(), Weekday::Saturday);
    assert_eq!(
        saturday.start_of_week(Weekday::Monday),
        Date::new(2026, 9, 21)
    );
    assert_eq!(
        saturday.start_of_week(Weekday::Sunday),
        Date::new(2026, 9, 20)
    );
    assert_eq!(saturday.start_of_week(Weekday::Saturday), saturday);
}
