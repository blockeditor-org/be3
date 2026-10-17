use super::*;

#[test]
fn the_ioctls_match_the_drivers_numbers() {
    assert_eq!(GLOBAL_REFRESH as u64, 0xc001_6440);
    assert_eq!(RECT_HINTS as u64, 0x4010_6443);
}
