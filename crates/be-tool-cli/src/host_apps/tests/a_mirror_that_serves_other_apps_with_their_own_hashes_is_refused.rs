use super::*;

/// A mirror serving another module with a hash list that matches it is refused: `be` checks
/// against the hashes it carries and never reads the mirror's list, so a mirror can fail to
/// serve an app but not substitute one.
#[test]
fn a_mirror_that_serves_other_apps_with_their_own_hashes_is_refused() {
    let substitute: &[u8] = b"a host component that sends its block elsewhere";
    let mirrored_list = serde_json::to_vec(&carried(&[("counter.wasm", substitute)]))
        .expect("the list serializes");
    let fake = Fake::new(&[
        ("counter.wasm", substitute),
        ("app-hashes.json", &mirrored_list),
    ]);
    let hashes = carried(&[("counter.wasm", b"the counter's module")]);
    let release = Release::new(BASE.to_owned(), cache(), hashes, |url: &str| fake.fetch(url));
    assert!(release.read("counter.wasm").is_err(), "the substitute is refused");
    assert_eq!(fake.fetches_of("app-hashes.json"), 0, "the mirror's list is never read");
}
