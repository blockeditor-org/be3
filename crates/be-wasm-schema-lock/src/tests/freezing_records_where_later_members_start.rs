use super::*;

#[test]
fn freezing_records_where_later_members_start() {
    let lock = frozen_person();
    assert_eq!(lock.types["Person"].frozen_from, Some(3));
    let grown = "#[derive(Schema)] #[schema(frozen)]
        struct Person { name: String, email: Option<String>, age: u32 }";
    let lock = lock_of(&[("a.rs", grown)], &lock).unwrap();
    assert_eq!(lock.types["Person"].frozen_from, Some(3));
    assert_eq!(lock.types["Person"].id("email"), Some(3));
}
