use super::*;

#[test]
fn right_shift_is_found_with_or_without_the_extended_flag() {
    assert_eq!(evdev(0xA1, ScanCode::new(0x36, false)), Some(54));
    assert_eq!(evdev(0xA1, ScanCode::new(0x36, true)), Some(54));
}
