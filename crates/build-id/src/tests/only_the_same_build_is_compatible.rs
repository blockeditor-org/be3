use super::*;

#[test]
fn only_the_same_build_is_compatible() {
    assert!(compatible("abc", "abc"));
    assert!(!compatible("abc", "abc+1f2e"));
    assert!(!compatible("unknown", "abc"));
}
