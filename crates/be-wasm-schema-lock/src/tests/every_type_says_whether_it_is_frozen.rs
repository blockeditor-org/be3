use super::*;

#[test]
fn every_type_says_whether_it_is_frozen() {
    for source in [
        "#[derive(Schema)] struct Person { name: String }",
        "#[derive(Schema)] #[schema(frozen, unfrozen)] struct Person { name: String }",
    ] {
        one_error(
            lock_of(&[("a.rs", source)], &Lock::default()),
            "says whether it is #[schema(frozen)]",
        );
    }
}
