use super::*;

#[test]
fn a_frozen_type_never_gives_a_retired_name_again() {
    let confirmed = Lock::parse(
        &frozen_person()
            .to_text()
            .replace("1 name: String", "1 retired name: String"),
    )
    .unwrap();
    one_error(
        lock_of(
            &[(
                "a.rs",
                "#[derive(Schema)] #[schema(frozen)] struct Person { age: u32, name: Option<String> }",
            )],
            &confirmed,
        ),
        "give the new member another name",
    );
}
