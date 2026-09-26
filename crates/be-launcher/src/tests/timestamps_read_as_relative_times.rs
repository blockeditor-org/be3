use super::*;

#[test]
fn timestamps_read_as_relative_times() {
    assert_eq!(parse_timestamp("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_timestamp("2026-09-25T23:30:47Z"), Some(1_790_379_047));
    assert_eq!(parse_timestamp("not a time"), None);
    let now = 1_000_000;
    assert_eq!(relative(now, now - 5), "just now");
    assert_eq!(relative(now, now - 60), "1 minute ago");
    assert_eq!(relative(now, now - 3 * 3600), "3 hours ago");
    assert_eq!(relative(now, now - 2 * 86_400), "2 days ago");
}
