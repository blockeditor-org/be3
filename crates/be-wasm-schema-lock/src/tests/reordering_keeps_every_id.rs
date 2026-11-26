use super::*;

#[test]
fn reordering_keeps_every_id() {
    let old = lock_of(&[("a.rs", PERSON)], &Lock::default()).unwrap();
    let reordered = "#[derive(Schema)] struct Person { age: u32, name: String }";
    let lock = lock_of(&[("a.rs", reordered)], &old).unwrap();
    assert_eq!(lock, old);
}
