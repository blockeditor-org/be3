use super::*;

#[test]
fn a_root_link_takes_libtest_from_beside_std() {
    let beside = arguments(
        "rust/lib/libcore-3.rlib rust/lib/libgetopts-4.rlib rust/lib/libstd-1.rlib \
         rust/lib/libtest-2.rlib rust/lib/libtest-2.rmeta",
    );
    let rewritten = with_test_harness(
        arguments("-flavor wasm sdk.o rust/lib/libstd-1.rlib rust/lib/libcore-3.rlib -o base.wasm"),
        &beside,
    );
    assert_eq!(
        rewritten,
        arguments(
            "-flavor wasm sdk.o rust/lib/libstd-1.rlib rust/lib/libgetopts-4.rlib \
             rust/lib/libtest-2.rlib rust/lib/libcore-3.rlib -o base.wasm"
        )
    );
}
