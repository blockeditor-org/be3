use super::*;

#[test]
fn format_datetime_utc_formats_readable_utc_string() {
    assert_eq!(format_datetime_utc(0), "1970-01-01 00:00");
    assert_eq!(format_datetime_utc(1_704_070_861), "2024-01-01 01:01");
}
