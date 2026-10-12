use super::*;

#[test]
fn a_plugin_link_has_the_sdk_thread_locals_first() {
    let mirror = Path::new("out.wasm.sdk-tls.o");
    let rewritten = with_mirror(arguments("-flavor wasm --export=x plugin.o -o out"), mirror);
    assert_eq!(
        rewritten,
        arguments("-flavor wasm --export=x out.wasm.sdk-tls.o plugin.o -o out")
    );
    let placed = map::ThreadLocals {
        inputs: vec!["out.wasm.sdk-tls.o:(.tbss.__be3_sdk_tls)".to_owned()],
        symbols: vec![map::Placed {
            name: "OWN".to_owned(),
            offset: 16,
            size: 40,
        }],
    };
    assert!(check_thread_locals(&placed, mirror, &sdk(&[])).unwrap_err().contains("more than the 32"));
}
