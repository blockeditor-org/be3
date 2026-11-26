use super::*;

#[test]
fn a_frozen_member_may_be_moved() {
    let moved = "be_wasm_schema::frozen!();
        #[derive(Schema)] struct Person { age: u32, name: String }";
    assert_eq!(
        lock_of(&[("a.rs", moved)], &frozen_person()).unwrap(),
        frozen_person()
    );
}
