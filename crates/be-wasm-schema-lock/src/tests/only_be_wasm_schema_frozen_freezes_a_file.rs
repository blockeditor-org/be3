use super::*;

#[test]
fn only_be_wasm_schema_frozen_freezes_a_file() {
    for (call, frozen) in [
        ("be_wasm_schema::frozen!();", true),
        ("frozen!();", true),
        ("other::frozen!();", false),
        ("be_wasm_schema::frozen_later!();", false),
    ] {
        let lock = lock_of(&[("a.rs", &format!("{call}\n{PERSON}"))], &Lock::default()).unwrap();
        assert_eq!(lock.types["Person"].is_frozen(), frozen, "{call}");
    }
}
