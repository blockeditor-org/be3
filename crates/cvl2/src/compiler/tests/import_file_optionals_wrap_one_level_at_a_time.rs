use super::*;

#[test]
fn import_file_optionals_wrap_one_level_at_a_time() {
    let nested = "    inner := std.Option(std.kw.int): std.kw.null\n    outer := std.Option(std.Option(std.kw.int)): inner";
    assert_eq!(
        build_file(&format!("{nested}\n    x := outer.?\n    -> \"x\"")),
        Ok("x".to_string())
    );
    assert_eq!(
        only_error(build_file(&format!(
            "{nested}\n    x := outer.?.?\n    -> \"x\""
        ))),
        "unwrapped std.kw.null with .?"
    );
    assert_eq!(
        only_error(build_file(
            "    x := std.Option(std.kw.int): \"a\"\n    -> \"x\""
        )),
        "String is not supported in slot: ?KwInt"
    );
    assert_eq!(
        only_error(build_file(
            "    x := std.Option(std.kw.int): std.kw.bool.true\n    -> \"x\""
        )),
        "expected ?KwInt, got KwBool"
    );
}
