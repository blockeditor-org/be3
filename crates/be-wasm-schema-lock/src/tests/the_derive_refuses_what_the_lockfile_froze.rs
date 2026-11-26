use super::*;

#[test]
fn the_derive_refuses_what_the_lockfile_froze() {
    let input: syn::DeriveInput =
        syn::parse_str("#[derive(Schema)] struct Person { name: String, age: i64 }").unwrap();
    let errors = numbered(&input, &frozen_person()).unwrap_err();
    assert!(errors[0].contains("Person.age is frozen"), "{errors:?}");
}
