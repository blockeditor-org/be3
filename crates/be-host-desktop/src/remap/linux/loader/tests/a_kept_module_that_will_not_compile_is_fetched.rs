use super::*;

#[test]
fn a_kept_module_that_will_not_compile_is_fetched() {
    let mut loader = Loader::default();
    loader.compiling(wanted("kept.wasm", b"a"));
    assert_eq!(
        loader.want(Some(wanted("kept.wasm", b"a"))),
        Step::Nothing,
        "the module this device kept is compiling as the workspace loads"
    );
    loader.failed();
    assert_eq!(
        loader.want(Some(wanted("kept.wasm", b"b"))),
        Step::Fetch {
            app: "be3.settings".to_owned(),
            module: "kept.wasm".to_owned(),
        }
    );
}
