use super::*;

#[test]
fn import_file_kw_else_must_follow_if_on_same_line() {
    assert_eq!(
        only_error(build_file(
            "    std.kw.if (.true) { }\n    .else { }\n    -> \"x\""
        )),
        ".else must follow the } of a std.kw.if on the same line"
    );
}
