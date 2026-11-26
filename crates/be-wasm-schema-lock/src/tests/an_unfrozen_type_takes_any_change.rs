use super::*;

#[test]
fn an_unfrozen_type_takes_any_change() {
    let old = lock_of(&[("a.rs", PERSON)], &Lock::default()).unwrap();
    let changed = "#[derive(Schema)] #[schema(unfrozen)] enum Person { Name(String), Age(u64) }";
    assert_eq!(
        lock_of(&[("a.rs", changed)], &old).unwrap().types["Person"].kind,
        Kind::Enum
    );
    let retyped = "#[derive(Schema)] #[schema(unfrozen)] struct Person {
        #[schema(critical)] name: Vec<u8>, age: u64 }";
    let lock = lock_of(&[("a.rs", retyped)], &old).unwrap();
    assert_eq!(
        lock.types["Person"].members[0].shape,
        ": Vec<u8> [critical]"
    );
    assert_eq!(lock.types["Person"].members[0].id, 1);
    assert!(lock_of(&[], &old).unwrap().types.is_empty());
}
