use super::*;

#[test]
fn a_file_is_refused_when_it_does_not_match_its_hash() {
    let cache = scratch("mismatch");
    let mut store = Store::default();
    let wanted = store.put(b"block-app.apk");
    let other = store.put(b"something else");
    let tampered = store.answers[&helper::file_url(&other.sha256)].clone();
    store
        .answers
        .insert(helper::file_url(&wanted.sha256), tampered);
    let tampered_manifest = manifest(vec![("block-app.apk", wanted.clone())]);

    let error = builds::fetch(&cache, &tampered_manifest, &mut store, &mut |_, _| {}).unwrap_err();

    assert!(error.contains(&other.sha256), "{error}");
    assert!(!builds::cached(&cache, &wanted.sha256).exists());

    let missing = manifest(vec![(
        "block-app.apk",
        helper::File {
            sha256: "b".repeat(64),
            ..wanted
        },
    )]);
    let error = builds::fetch(&cache, &missing, &mut store, &mut |_, _| {}).unwrap_err();
    assert!(error.contains("404"), "{error}");
    let _ = std::fs::remove_dir_all(&cache);
}
