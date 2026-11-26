use super::*;

#[test]
fn a_schema_type_has_one_name_in_its_crate() {
    one_error(
        lock_of(&[("a.rs", PERSON), ("b.rs", PERSON)], &Lock::default()),
        "in both a.rs and b.rs",
    );
}
