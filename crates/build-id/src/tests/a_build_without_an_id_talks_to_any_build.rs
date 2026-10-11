use super::*;

#[test]
fn a_build_without_an_id_talks_to_any_build() {
    assert!(compatible("abc", "abc"));
    assert!(!compatible("abc", "abc+1f2e"));
    assert!(compatible(UNKNOWN, "abc"));
    assert!(compatible("abc+1f2e", UNKNOWN));
}
