//! Times as the editor says them, from whole seconds since the Unix epoch.

use be_sdk_beui::beui::datetime::Date;

pub(crate) const DAY: i64 = 24 * 60 * 60;

/// The wall clock in seconds: the host's, which a test pins.
pub(crate) fn now() -> i64 {
    be_sdk_beui::beui::reactive::wall_clock().as_secs() as i64
}

/// The date of `seconds`, as `2 March 2027`.
pub(crate) fn date(seconds: i64) -> String {
    let date = Date::from_days(seconds.div_euclid(DAY));
    format!("{} {} {}", date.day, date.month_name(), date.year)
}

/// How long ago `then` was, at `now`: `just now`, `5 minutes ago`, `2 hours ago`.
pub(crate) fn ago(then: i64, now: i64) -> String {
    let seconds = (now - then).max(0);
    let (count, unit) = match seconds {
        0..60 => return "just now".to_owned(),
        60..3600 => (seconds / 60, "minute"),
        3600..86_400 => (seconds / 3600, "hour"),
        _ => (seconds / DAY, "day"),
    };
    match count {
        1 => format!("1 {unit} ago"),
        count => format!("{count} {unit}s ago"),
    }
}
