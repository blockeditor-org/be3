use super::*;

#[test]
fn a_module_changed_while_another_compiles_is_fetched_and_the_other_dropped() {
    let mut loader = Loader::default();
    loader.want(Some(wanted("first.wasm", b"a")));
    assert!(loader.fetched("be3.settings", "first.wasm").is_some());

    assert_eq!(
        loader.want(Some(wanted("second.wasm", b"b"))),
        Step::Fetch {
            app: "be3.settings".to_owned(),
            module: "second.wasm".to_owned(),
        }
    );
    assert!(
        !loader.wants("be3.settings", "first.wasm"),
        "the first module, once compiled, is not installed"
    );
    assert!(loader.fetching("be3.settings", "second.wasm"));
}
