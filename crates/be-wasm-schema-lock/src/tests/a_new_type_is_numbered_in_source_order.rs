use super::*;

#[test]
fn a_new_type_is_numbered_in_source_order() {
    let lock = lock_of(&[("a.rs", PERSON)], &Lock::default()).unwrap();
    assert_eq!(
        ids(&lock, "Person"),
        [(1, "name".into(), false), (2, "age".into(), false)]
    );
    assert_eq!(lock.types["Person"].members[0].shape, ": String");
    assert!(!lock.types["Person"].is_frozen());
}
