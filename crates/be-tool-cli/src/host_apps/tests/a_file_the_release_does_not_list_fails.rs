use super::*;

/// A file the release's hashes do not list fails without being downloaded, and so does every
/// file of a release that has no list.
#[test]
fn a_file_the_release_does_not_list_fails() {
    let fake = Fake::new(&[("unlisted.wasm", b"anything")], &[]);
    let release = fake.release(cache());
    let error = release.read("unlisted.wasm").expect_err("an unlisted file fails");
    assert!(error.contains("lists no unlisted.wasm"), "{error}");
    assert_eq!(fake.fetches_of("unlisted.wasm"), 0, "not downloaded");

    let bare = Release::new(
        ReleaseApps {
            url: "https://mirror.test/data/old/web".to_owned(),
            sha256: fake.apps.sha256.clone(),
        },
        cache(),
        |url: &str| fake.fetch(url),
    );
    let error = bare.read("counter.wasm").expect_err("a release with no hashes fails");
    assert!(error.contains(HASHES), "{error}");
}
