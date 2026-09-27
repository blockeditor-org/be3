use super::*;

#[test]
fn adding_months_keeps_the_day_within_the_month_it_lands_in() {
    assert_eq!(Date::new(2025, 1, 31).add_months(1), Date::new(2025, 2, 28));
    assert_eq!(Date::new(2024, 1, 31).add_months(1), Date::new(2024, 2, 29));
    assert_eq!(
        Date::new(2025, 12, 15).add_months(1),
        Date::new(2026, 1, 15)
    );
    assert_eq!(
        Date::new(2025, 1, 15).add_months(-1),
        Date::new(2024, 12, 15)
    );
    assert_eq!(Date::new(2024, 2, 29).add_years(1), Date::new(2025, 2, 28));
}
