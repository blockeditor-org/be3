use super::*;

#[test]
fn a_date_round_trips_through_its_day_number() {
    assert_eq!(Date::EPOCH.days(), 0);
    assert_eq!(Date::EPOCH.weekday(), Weekday::Thursday);
    for days in [-800_000, -1, 0, 59, 11_016, 20_723, 2_932_896] {
        assert_eq!(Date::from_days(days).days(), days);
    }
    assert_eq!(Date::new(2024, 2, 29).add_days(1), Date::new(2024, 3, 1));
    assert_eq!(
        DateTime::from_unix(DateTime::new(Date::new(2026, 9, 27), Time::new(13, 45)).to_unix()),
        DateTime::new(Date::new(2026, 9, 27), Time::new(13, 45))
    );
}
