use std::time::{SystemTime, UNIX_EPOCH};

use beui::datetime::Date;

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const MONTH: i64 = 30 * DAY;
const YEAR: i64 = 365 * DAY;

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

pub(crate) fn parse_timestamp(text: &str) -> Option<i64> {
    let (date, time) = text.split_once('T')?;
    let mut date = date.splitn(3, '-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let time = time.trim_end_matches('Z');
    let time = time.split(['.', '+']).next()?;
    let mut time = time.splitn(3, ':').map(str::parse::<i64>);
    let (hour, minute, second) = (time.next()?.ok()?, time.next()?.ok()?, time.next()?.ok()?);
    Some(
        Date::new(year as i32, month as u8, day as u8).days() * DAY
            + hour * HOUR
            + minute * MINUTE
            + second,
    )
}

pub(crate) fn relative(now: i64, then: i64) -> String {
    let elapsed = (now - then).max(0);
    let (count, unit) = match elapsed {
        elapsed if elapsed < MINUTE => return "just now".to_owned(),
        elapsed if elapsed < HOUR => (elapsed / MINUTE, "minute"),
        elapsed if elapsed < DAY => (elapsed / HOUR, "hour"),
        elapsed if elapsed < MONTH => (elapsed / DAY, "day"),
        elapsed if elapsed < YEAR => (elapsed / MONTH, "month"),
        elapsed => (elapsed / YEAR, "year"),
    };
    if count == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
}
