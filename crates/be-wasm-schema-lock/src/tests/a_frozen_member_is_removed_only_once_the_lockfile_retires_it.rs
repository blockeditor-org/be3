use super::*;

#[test]
fn a_frozen_member_is_removed_only_once_the_lockfile_retires_it() {
    let removed = "be_wasm_schema::frozen!();
        #[derive(Schema)] struct Person { age: u32 }";
    one_error(
        lock_of(&[("a.rs", removed)], &frozen_person()),
        "confirm it there by replacing `1 name: String` with `1 name retired`",
    );
    let confirmed = Lock::parse(
        &frozen_person()
            .to_text()
            .replace("1 name: String", "1 name retired"),
    )
    .unwrap();
    let lock = lock_of(&[("a.rs", removed)], &confirmed).unwrap();
    assert_eq!(
        ids(&lock, "Person"),
        [(1, "name".into(), true), (2, "age".into(), false)]
    );
    // The derive refuses the removal until then too.
    let input: syn::DeriveInput = syn::parse_str("struct Person { age: u32 }").unwrap();
    assert!(numbered(&input, &frozen_person()).is_err());
    assert!(numbered(&input, &confirmed).is_ok());
}
