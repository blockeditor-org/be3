use super::*;

#[test]
fn a_frozen_type_stays_frozen() {
    one_error(
        lock_of(&[("a.rs", PERSON)], &frozen_person()),
        "no longer calls be_wasm_schema::frozen!()",
    );
    one_error(
        lock_of(&[("a.rs", "be_wasm_schema::frozen!();")], &frozen_person()),
        "Person is frozen in schema.lock and no longer in the source",
    );
}
