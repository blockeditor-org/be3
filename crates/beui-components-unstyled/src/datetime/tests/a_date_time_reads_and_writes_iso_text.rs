use super::*;

#[test]
fn a_date_time_reads_and_writes_iso_text() {
    assert_eq!(DateTime::from_unix(0).iso(), "1970-01-01 00:00");
    assert_eq!(DateTime::from_unix(1_704_070_861).iso(), "2024-01-01 01:01");
    let leap = DateTime::parse_iso(" 2024-02-29 23:59 ").unwrap();
    assert_eq!(DateTime::from_unix(leap.to_unix()), leap);
    for invalid in [
        "2023-02-29 10:00",
        "2024-13-01 10:00",
        "2024-01-01 24:00",
        "2024-01-01",
        "2024-01-01 10:00:00",
        "0-01-01 10:00",
    ] {
        assert_eq!(DateTime::parse_iso(invalid), None, "{invalid}");
    }
}
