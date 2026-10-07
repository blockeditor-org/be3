use super::*;

#[test]
fn import_file_kw_if_condition_must_be_kw_bool() {
    assert_eq!(
        only_error(build_file(
            "    std.kw.if (std.kw.int: 1) { }\n    -> \"x\""
        )),
        "expected KwBool, got KwInt"
    );
    assert_eq!(
        build_file("    std.kw.if (std.kw.true != std.kw.false) { } .else { }\n    -> \"x\""),
        Ok("x".to_string())
    );
}
