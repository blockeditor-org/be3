use super::*;

#[test]
fn a_config_changed_while_the_module_compiles_waits_for_it() {
    let mut loader = Loader::default();
    loader.installed(wanted("old.wasm", b"old"));
    loader.want(Some(wanted("new.wasm", b"a")));
    assert_eq!(
        loader.fetched("be3.settings", "new.wasm"),
        Some(wanted("new.wasm", b"a"))
    );

    assert_eq!(
        loader.want(Some(wanted("new.wasm", b"b"))),
        Step::Nothing,
        "the old module is not given the new module's config while the new one compiles"
    );
    assert!(loader.wants("be3.settings", "new.wasm"));
    assert_eq!(loader.wanted(), Some(&wanted("new.wasm", b"b")));
    loader.installed(wanted("new.wasm", b"b"));
    assert_eq!(
        loader.want(Some(wanted("new.wasm", b"c"))),
        Step::Configure(b"c".to_vec())
    );
}
