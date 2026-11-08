use std::time::Duration;

use crate::parse_duration;

#[test]
fn durations_are_seconds_minutes_or_hours() {
    assert_eq!(parse_duration("90"), Ok(Duration::from_secs(90)));
    assert_eq!(parse_duration("90s"), Ok(Duration::from_secs(90)));
    assert_eq!(parse_duration("20m"), Ok(Duration::from_secs(1200)));
    assert_eq!(parse_duration("1h"), Ok(Duration::from_secs(3600)));
    assert!(parse_duration("m").is_err());
    assert!(parse_duration("2d").is_err());
}
