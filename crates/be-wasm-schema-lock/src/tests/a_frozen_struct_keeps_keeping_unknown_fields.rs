use super::*;

#[test]
fn a_frozen_struct_keeps_keeping_unknown_fields() {
    let keeping = "#[derive(Schema)] #[schema(frozen)]
        struct Person { name: String, #[schema(unknown)] rest: Unknown }";
    let old = lock_of(&[("a.rs", keeping)], &Lock::default()).unwrap();
    assert!(old.types["Person"].keeps_unknown);
    one_error(
        lock_of(
            &[(
                "a.rs",
                "#[derive(Schema)] #[schema(frozen)] struct Person { name: String }",
            )],
            &old,
        ),
        "keeps its #[schema(unknown)] field",
    );
}
