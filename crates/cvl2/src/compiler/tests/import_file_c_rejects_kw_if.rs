use super::*;

#[test]
fn import_file_c_rejects_kw_if() {
    assert_eq!(
        only_error(build_c_fn("  std.kw.if (.true) { }\n  -> a")),
        "std.kw.if is not supported in C"
    );
}
