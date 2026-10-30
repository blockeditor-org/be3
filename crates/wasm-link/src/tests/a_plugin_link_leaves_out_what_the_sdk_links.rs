use super::*;

#[test]
fn a_plugin_link_leaves_out_what_the_sdk_links() {
    let rewritten = plugin_arguments(
        arguments(
            "-flavor wasm -z stack-size=1048576 --stack-first --import-memory \
             --export=__heap_base plugin.o deps/libbeui-1.rlib deps/libimage-2.rlib -l c -lc++abi \
             sysroot/libsetjmp.a -o out.wasm",
        ),
        &sdk(&["libbeui-1.rlib", "-lc", "-lc++abi", "libsetjmp.a"]).linked,
    );
    let expected: Vec<String> = arguments("-flavor wasm plugin.o deps/libimage-2.rlib -o out.wasm")
        .into_iter()
        .chain(PLUGIN_FLAGS.map(str::to_owned))
        .collect();
    assert_eq!(rewritten, expected);
}
