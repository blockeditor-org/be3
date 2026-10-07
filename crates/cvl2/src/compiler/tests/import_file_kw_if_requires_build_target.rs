use super::*;

#[test]
fn import_file_kw_if_requires_build_target() {
    assert_eq!(
        only_error(build_c_fn("  std.kw.if (.true) { }\n  -> a")),
        "std.kw.if is only available when compiling to the build, not C"
    );
}
