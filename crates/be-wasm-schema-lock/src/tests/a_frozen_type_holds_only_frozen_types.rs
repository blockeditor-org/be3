use super::*;

#[test]
fn a_frozen_type_holds_only_frozen_types() {
    let files = [
        (
            "frozen.rs",
            "be_wasm_schema::frozen!();
            #[derive(Schema)] struct Team { members: Vec<Person> }",
        ),
        ("a.rs", PERSON),
    ];
    one_error(
        lock_of(&files, &Lock::default()),
        "Team is frozen and holds Person, which is not",
    );
}
