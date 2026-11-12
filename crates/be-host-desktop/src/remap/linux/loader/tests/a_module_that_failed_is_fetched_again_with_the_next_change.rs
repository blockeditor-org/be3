use super::*;

#[test]
fn a_module_that_failed_is_fetched_again_with_the_next_change() {
    let mut loader = Loader::default();
    let fetch = Step::Fetch {
        app: "be3.settings".to_owned(),
        module: "remapper.wasm".to_owned(),
    };
    assert_eq!(loader.want(Some(wanted("remapper.wasm", b"a"))), fetch);
    assert!(loader.fetching("be3.settings", "remapper.wasm"));
    loader.failed();
    assert_eq!(
        loader.want(Some(wanted("remapper.wasm", b"b"))),
        fetch,
        "a module that never arrived is asked for again, not waited on forever"
    );

    assert!(loader.fetched("be3.settings", "remapper.wasm").is_some());
    loader.failed();
    assert_eq!(
        loader.want(Some(wanted("remapper.wasm", b"c"))),
        fetch,
        "a module that would not load is not reconfigured as if it ran"
    );
}
