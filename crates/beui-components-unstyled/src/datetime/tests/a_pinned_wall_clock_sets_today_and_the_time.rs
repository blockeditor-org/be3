use super::*;
use std::time::Duration;

#[test]
fn a_pinned_wall_clock_sets_today_and_the_time() {
    let noon = Date::new(2024, 2, 29).days() as u64 * SECONDS_PER_DAY as u64 + 12 * 3600 + 34 * 60;
    let now = beui_view::reactive::now();
    beui_view::reactive::pin_wall_clock(Some((now, Duration::from_secs(noon))));

    assert_eq!(Date::today(), Date::new(2024, 2, 29));
    assert_eq!(Time::now(), Time::new(12, 34));

    beui_view::reactive::pin_wall_clock(None);
}
