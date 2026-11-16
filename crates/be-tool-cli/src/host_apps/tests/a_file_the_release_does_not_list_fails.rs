use super::*;

/// A file `be`'s release has no hash for fails without being downloaded.
#[test]
fn a_file_the_release_does_not_list_fails() {
    let fake = Fake::new(&[("unlisted.wasm", b"anything")]);
    let release = Release::new(BASE.to_owned(), cache(), HashMap::new(), |url: &str| {
        fake.fetch(url)
    });
    let error = release.read("unlisted.wasm").expect_err("an unlisted file fails");
    assert!(error.contains("has no unlisted.wasm"), "{error}");
    assert_eq!(fake.fetches_of("unlisted.wasm"), 0, "not downloaded");
}
