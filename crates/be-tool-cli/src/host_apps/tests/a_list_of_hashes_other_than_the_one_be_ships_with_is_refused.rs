use super::*;

/// The mirror serving a list of hashes other than the one `be`'s `build-info.json` names, as it
/// would to slip in other apps with hashes of their own, fails every read, naming the list and
/// `build-info.json`, and downloads no app.
#[test]
fn a_list_of_hashes_other_than_the_one_be_ships_with_is_refused() {
    let other: &[u8] = b"another module";
    let fake = Fake::new(&[("counter.wasm", other)], &[("counter.wasm", other)]);
    let release = Release::new(
        ReleaseApps {
            url: BASE.to_owned(),
            sha256: sha256(b"the list this be was deployed with"),
        },
        cache(),
        |url: &str| fake.fetch(url),
    );
    let error = release.read("counter.wasm").expect_err("another list fails");
    assert!(error.contains(HASHES), "{error}");
    assert!(error.contains(be_lib_build_id::FILE), "{error}");
    assert_eq!(fake.fetches_of("counter.wasm"), 0, "no app downloaded");
}
