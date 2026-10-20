use super::*;

#[test]
fn a_module_fetched_after_the_remapper_changed_is_not_installed() {
    let mut loader = Loader::default();
    loader.installed(wanted("old.wasm", b"cached"));
    assert_eq!(
        loader.want(Some(wanted("old.wasm", b"cached"))),
        Step::Nothing,
        "the remapper this device kept runs on as the workspace loads"
    );

    loader.want(Some(wanted("new.wasm", b"a")));
    loader.want(Some(wanted("other.wasm", b"a")));
    assert_eq!(loader.fetched("be3.settings", "new.wasm"), None);
    assert_eq!(
        loader.fetched("be3.settings", "other.wasm"),
        Some(wanted("other.wasm", b"a"))
    );
}
