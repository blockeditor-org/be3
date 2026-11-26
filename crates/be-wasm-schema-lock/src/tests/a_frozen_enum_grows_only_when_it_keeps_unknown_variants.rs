use super::*;

#[test]
fn a_frozen_enum_grows_only_when_it_keeps_unknown_variants() {
    let closed = "be_wasm_schema::frozen!();
        #[derive(Schema)] enum Shape { Dot, Circle(f64) }";
    let old = lock_of(&[("a.rs", closed)], &Lock::default()).unwrap();
    let grown = "be_wasm_schema::frozen!();
        #[derive(Schema)] enum Shape { Dot, Circle(f64), Square { side: f64 } }";
    one_error(
        lock_of(&[("a.rs", grown)], &old),
        "no #[schema(unknown)] case",
    );

    let open = "be_wasm_schema::frozen!();
        #[derive(Schema)] enum Shape { Dot, Circle(f64), #[schema(unknown)] Other(Unknown) }";
    let old = lock_of(&[("a.rs", open)], &Lock::default()).unwrap();
    assert!(old.types["Shape"].open);
    let grown = "be_wasm_schema::frozen!();
        #[derive(Schema)] enum Shape {
            Dot, Circle(f64), Square { side: f64 }, #[schema(unknown)] Other(Unknown)
        }";
    let lock = lock_of(&[("a.rs", grown)], &old).unwrap();
    assert_eq!(lock.types["Shape"].id("Square"), Some(3));
    assert_eq!(lock.types["Shape::Square"].id("side"), Some(1));
    assert!(lock.types["Shape::Square"].is_frozen());

    let closed_again = "be_wasm_schema::frozen!();
        #[derive(Schema)] enum Shape { Dot, Circle(f64), Square { side: f64 } }";
    one_error(
        lock_of(&[("a.rs", closed_again)], &lock),
        "keeps its #[schema(unknown)] case",
    );
}
