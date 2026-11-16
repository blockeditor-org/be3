use super::*;

/// A file whose bytes are not what the release lists fails, naming the file, and is not kept.
#[test]
fn a_release_file_that_does_not_match_its_hash_fails_naming_it() {
    let fake = Fake::new(
        &[("counter.wasm", b"what the mirror served")],
        &[("counter.wasm", b"what the release built")],
    );
    let cache = cache();
    let release = Release::new(BASE.to_owned(), cache.clone(), |url: &str| fake.fetch(url));
    let error = release.read("counter.wasm").expect_err("a mismatch fails");
    assert!(error.contains("counter.wasm"), "{error}");
    assert!(error.contains(&sha256(b"what the release built")), "{error}");
    assert_eq!(
        std::fs::read_dir(&cache).map(Iterator::count).unwrap_or(0),
        0,
        "nothing kept"
    );
}
