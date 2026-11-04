use super::*;

#[test]
fn bytes_read_in_decimal_units_to_two_figures() {
    assert_eq!(bytes_label(0), "0 bytes");
    assert_eq!(bytes_label(1), "1 byte");
    assert_eq!(bytes_label(999), "999 bytes");
    assert_eq!(bytes_label(1_000), "1.0 KB");
    assert_eq!(bytes_label(3_400_000), "3.4 MB");
    assert_eq!(bytes_label(12_345_678), "12 MB");
    assert_eq!(bytes_label(999_999_999), "1.0 GB");
    assert_eq!(bytes_label(10_737_418_240), "11 GB");
    assert_eq!(bytes_label(u64::MAX), "18447 PB");
}
