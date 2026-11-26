use super::*;

#[test]
fn freezing_records_where_later_members_start() {
    let frozen = format!("be_wasm_schema::frozen!();\n{PERSON}");
    let lock = lock_of(&[("a.rs", &frozen)], &Lock::default()).unwrap();
    assert_eq!(lock.types["Person"].frozen_from, Some(3));
    let grown = "be_wasm_schema::frozen!();
        #[derive(Schema)] struct Person { name: String, email: Option<String>, age: u32 }";
    let lock = lock_of(&[("a.rs", grown)], &lock).unwrap();
    assert_eq!(lock.types["Person"].frozen_from, Some(3));
    assert_eq!(lock.types["Person"].id("email"), Some(3));
}
