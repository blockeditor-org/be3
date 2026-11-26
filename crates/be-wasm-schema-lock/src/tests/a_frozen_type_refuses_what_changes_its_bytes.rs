use super::*;

#[test]
fn a_frozen_type_refuses_what_changes_its_bytes() {
    for (source, error) in [
        (
            "struct Person { name: String, age: u64 }",
            "Person.age is frozen as `age: u32` and cannot become `age: u64`",
        ),
        (
            "struct Person { name: String, #[schema(critical)] age: u32 }",
            "cannot become `age: u32 [critical]`",
        ),
        (
            "struct Person { #[schema(default = \"x\".into())] name: String, age: u32 }",
            "cannot become `name: String [default = \"x\".into()]`",
        ),
        (
            "struct Person { #[schema(no_copy)] name: String, age: u32 }",
            "cannot become `name: String [no_copy]`",
        ),
    ] {
        let source = format!("#[derive(Schema)] #[schema(frozen)] {source}");
        one_error(lock_of(&[("a.rs", &source)], &frozen_person()), error);
    }
}
