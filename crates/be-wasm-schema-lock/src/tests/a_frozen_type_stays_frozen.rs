use super::*;

#[test]
fn a_frozen_type_stays_frozen() {
    one_error(
        lock_of(&[("a.rs", PERSON)], &frozen_person()),
        "is marked #[schema(unfrozen)]: a frozen type stays frozen",
    );
    one_error(
        lock_of(&[("a.rs", "")], &frozen_person()),
        "Person is frozen in schema.lock and no longer in the source",
    );
}
