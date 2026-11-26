use super::*;

/// Serving a release downloads nothing: discovery asks it for `build-info.json` as it starts,
/// which `be` has beside it, so a command that needs no block type (help, whoami) fetches
/// nothing, and the first manifest or type module read fetches the list and that file alone.
#[test]
fn nothing_is_downloaded_until_an_app_file_is_read() {
    let manifest: &[u8] = b"{\"id\": \"be3.counter\"}";
    let types: &[u8] = b"the counter's type module";
    let files = [
        ("be3.counter.app.json", manifest),
        ("counter_types.wasm", types),
    ];
    let fake = Fake::new(&files, &files);
    let release = fake.release(cache());
    assert!(release.read(be_lib_build_id::FILE).is_err());
    assert_eq!(fake.fetches_of(HASHES), 0, "nothing was downloaded");

    assert_eq!(release.read("counter_types.wasm").as_deref(), Ok(types));
    assert_eq!(fake.fetches_of(HASHES), 1);
    assert_eq!(
        fake.fetches_of("be3.counter.app.json"),
        0,
        "only the file read"
    );
}
