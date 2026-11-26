use super::*;

#[test]
fn a_frozen_enum_says_whether_it_may_grow() {
    one_error(
        lock_of(
            &[(
                "a.rs",
                "#[derive(Schema)] #[schema(frozen)] enum Shape { Dot, Circle(f64) }",
            )],
            &Lock::default(),
        ),
        "says whether a later build may add variants",
    );
    let closed = "#[derive(Schema)] #[schema(frozen, closed)] enum Shape { Dot, Circle(f64) }";
    let old = lock_of(&[("a.rs", closed)], &Lock::default()).unwrap();
    assert!(old.types["Shape"].closed);
    let grown = "#[derive(Schema)] #[schema(frozen, closed)]
        enum Shape { Dot, Circle(f64), Square { side: f64 } }";
    one_error(
        lock_of(&[("a.rs", grown)], &old),
        "no #[schema(unknown)] case",
    );

    let open = "#[derive(Schema)] #[schema(frozen)]
        enum Shape { Dot, Circle(f64), #[schema(unknown)] Other(UnknownVariant) }";
    let old = lock_of(&[("a.rs", open)], &Lock::default()).unwrap();
    assert!(old.types["Shape"].open);
    let grown = "#[derive(Schema)] #[schema(frozen)] enum Shape {
        Dot, Circle(f64), Square { side: f64 }, #[schema(unknown)] Other(UnknownVariant)
    }";
    let lock = lock_of(&[("a.rs", grown)], &old).unwrap();
    assert_eq!(lock.types["Shape"].id("Square"), Some(3));
    assert_eq!(lock.types["Shape::Square"].id("side"), Some(1));
    assert!(lock.types["Shape::Square"].is_frozen());
    one_error(
        lock_of(&[("a.rs", closed)], &lock),
        "keeps its #[schema(unknown)] case",
    );
    // Which variant is the default is part of its bytes, too.
    let defaulted = "#[derive(Schema)] #[schema(frozen)] enum Shape {
        #[schema(default)] Dot, Circle(f64), Square { side: f64 }, #[schema(unknown)] Other(UnknownVariant)
    }";
    one_error(
        lock_of(&[("a.rs", defaulted)], &lock),
        "cannot become `Dot [default]`",
    );
}
