use super::*;

#[test]
fn memory_types_prefer_device_local() {
    assert_eq!(memory_type(0b1010, |index| index == 3), Some(3));
    assert_eq!(memory_type(0b1010, |index| index == 2), Some(1));
    assert_eq!(memory_type(0, |_| true), None);
}
