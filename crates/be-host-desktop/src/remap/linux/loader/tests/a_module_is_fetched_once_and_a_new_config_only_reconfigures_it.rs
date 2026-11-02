use super::*;

#[test]
fn a_module_is_fetched_once_and_a_new_config_only_reconfigures_it() {
    let mut loader = Loader::default();
    assert_eq!(
        loader.want(Some(wanted("remapper.wasm", b"a"))),
        Step::Fetch {
            plugin: "be3.settings".to_owned(),
            module: "remapper.wasm".to_owned(),
        }
    );
    assert_eq!(
        loader.want(Some(wanted("remapper.wasm", b"b"))),
        Step::Nothing,
        "the module is already on its way"
    );
    assert_eq!(
        loader.fetched("be3.settings", "remapper.wasm"),
        Some(wanted("remapper.wasm", b"b")),
        "what arrives runs the config wanted by then"
    );
    assert_eq!(
        loader.want(Some(wanted("remapper.wasm", b"c"))),
        Step::Configure(b"c".to_vec())
    );
    assert_eq!(
        loader.want(Some(wanted("remapper.wasm", b"c"))),
        Step::Nothing
    );
    assert_eq!(loader.want(None), Step::Stop);
    assert!(matches!(
        loader.want(Some(wanted("remapper.wasm", b"c"))),
        Step::Fetch { .. }
    ));
}
