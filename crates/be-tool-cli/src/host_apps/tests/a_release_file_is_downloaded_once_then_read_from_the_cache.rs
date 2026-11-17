use super::*;

/// A file read from a release is downloaded, checked and kept under its hash, so a second
/// reader, even a new `be` with the same cache, reads it without downloading it again.
#[test]
fn a_release_file_is_downloaded_once_then_read_from_the_cache() {
    let module: &[u8] = b"the counter's module";
    let fake = Fake::new(&[("counter.wasm", module)], &[("counter.wasm", module)]);
    let cache = cache();
    let release = fake.release(cache.clone());
    assert_eq!(release.read("counter.wasm").as_deref(), Ok(module));
    assert_eq!(release.read("counter.wasm").as_deref(), Ok(module));
    let again = fake.release(cache.clone());
    assert_eq!(again.read("counter.wasm").as_deref(), Ok(module));
    assert_eq!(fake.fetches_of("counter.wasm"), 1, "downloaded once");
    assert!(cache.join(sha256(module)).is_file(), "kept under its hash");
}
