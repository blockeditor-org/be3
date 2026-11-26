use super::*;

#[test]
fn a_rename_of_a_frozen_member_is_made_in_the_lockfile() {
    let renamed =
        "#[derive(Schema)] #[schema(frozen)] struct Person { full_name: String, age: u32 }";
    one_error(
        lock_of(&[("a.rs", renamed)], &frozen_person()),
        "if it was renamed, rename it in schema.lock",
    );
    let edited = Lock::parse(&frozen_person().to_text().replace(" name:", " full_name:")).unwrap();
    let lock = lock_of(&[("a.rs", renamed)], &edited).unwrap();
    assert_eq!(lock.types["Person"].id("full_name"), Some(1));
}
