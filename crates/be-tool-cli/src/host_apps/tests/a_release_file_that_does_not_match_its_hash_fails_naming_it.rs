use super::*;

/// A file whose bytes are not what the release lists fails, naming the file, and is not kept.
#[test]
fn a_release_file_that_does_not_match_its_hash_fails_naming_it() {
    let fake = Fake::new(
        &[("counter.wasm", b"what the mirror served")],
        &[("counter.wasm", b"what the release built")],
    );
    let cache = cache();
    let release = fake.release(cache.clone());
    let error = release.read("counter.wasm").expect_err("a mismatch fails");
    assert!(error.contains("counter.wasm"), "{error}");
    assert!(error.contains(&sha256(b"what the release built")), "{error}");
    assert!(
        !cache.join(sha256(b"what the mirror served")).exists()
            && !cache.join(sha256(b"what the release built")).exists(),
        "not kept"
    );
}
