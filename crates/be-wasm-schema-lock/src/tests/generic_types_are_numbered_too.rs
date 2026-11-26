use super::*;

#[test]
fn generic_types_are_numbered_too() {
    let source = "#[derive(Schema)] #[schema(unfrozen)] struct Pair<T> { first: T, second: T }";
    let lock = lock_of(&[("a.rs", source)], &Lock::default()).unwrap();
    assert_eq!(lock.types["Pair"].members[1].shape, ": T");
}
