use super::*;

#[test]
fn an_app_link_leaves_out_what_its_libraries_link() {
    let rewritten = app_arguments(
        arguments(
            "-flavor wasm -z stack-size=1048576 --stack-first --import-memory \
             --export=__heap_base app.o deps/libbeui-1.rlib deps/libimage-2.rlib -l c -lc++abi \
             sysroot/libsetjmp.a -o out.wasm",
        ),
        &HashSet::from(["libbeui-1.rlib", "-lc", "-lc++abi", "libsetjmp.a"].map(str::to_owned)),
    );
    let expected: Vec<String> = arguments("-flavor wasm app.o deps/libimage-2.rlib -o out.wasm")
        .into_iter()
        .chain(APP_FLAGS.map(str::to_owned))
        .collect();
    assert_eq!(rewritten, expected);
}
