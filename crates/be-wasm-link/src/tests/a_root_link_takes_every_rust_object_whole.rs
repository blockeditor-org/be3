use super::*;

#[test]
fn a_root_link_takes_every_rust_object_whole() {
    let (rewritten, linked) = root_arguments(arguments(
        "-flavor wasm sdk.o deps/libbeui-1.rlib -l c sysroot/libsetjmp.a -o sdk.wasm",
    ));
    assert_eq!(
        &rewritten[..11],
        arguments(
            "-flavor wasm --whole-archive sdk.o --no-whole-archive --whole-archive \
             deps/libbeui-1.rlib --no-whole-archive -l c sysroot/libsetjmp.a"
        )
    );
    assert!(rewritten.contains(&"--export-all".to_owned()));
    assert!(rewritten.contains(&"--undefined=sinf".to_owned()));
    assert_eq!(linked, ["libbeui-1.rlib", "-lc", "libsetjmp.a"]);
}
