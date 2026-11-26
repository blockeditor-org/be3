use super::*;

#[test]
fn a_frozen_type_refuses_a_retyped_field() {
    let retyped = "be_wasm_schema::frozen!();
        #[derive(Schema)] struct Person { name: String, age: u64 }";
    one_error(
        lock_of(&[("a.rs", retyped)], &frozen_person()),
        "Person.age is frozen as `age: u32`",
    );
}
