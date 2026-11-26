use super::*;

#[test]
fn the_derive_numbers_as_the_next_lockfile_will() {
    let old = lock_of(&[("a.rs", PERSON)], &Lock::default()).unwrap();
    let changed = "#[derive(Schema)] struct Person { email: String, age: u32, name: String }";
    let next = lock_of(&[("a.rs", changed)], &old).unwrap();
    let input: syn::DeriveInput = syn::parse_str(changed).unwrap();
    let numbered = numbered(&input, &old).unwrap();
    assert_eq!(numbered[0].1, next.types["Person"]);
    // And the lockfile it writes is where it stops.
    assert_eq!(lock_of(&[("a.rs", changed)], &next).unwrap(), next);
}
