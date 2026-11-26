use super::*;

#[test]
fn reordering_keeps_every_id() {
    let old = lock_of(&[("a.rs", PERSON)], &Lock::default()).unwrap();
    let reordered =
        "#[derive(Schema)] #[schema(unfrozen)] struct Person { age: u32, name: String }";
    assert_eq!(lock_of(&[("a.rs", reordered)], &old).unwrap(), old);
}
