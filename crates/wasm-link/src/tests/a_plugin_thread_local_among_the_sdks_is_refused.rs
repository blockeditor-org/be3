use super::*;

#[test]
fn a_plugin_thread_local_among_the_sdks_is_refused() {
    let placed = map::ThreadLocals {
        inputs: vec!["out.wasm.sdk-tls.o:(.tbss.__be3_sdk_tls)".to_owned()],
        symbols: vec![map::Placed {
            name: "OWN".to_owned(),
            offset: 4,
            size: 4,
        }],
    };
    let error =
        check_thread_locals(&placed, Path::new("out.wasm.sdk-tls.o"), &sdk(&[])).unwrap_err();
    assert!(error.contains("among the SDK's"));
}
