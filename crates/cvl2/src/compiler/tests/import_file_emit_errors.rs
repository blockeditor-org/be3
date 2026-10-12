use super::*;

#[test]
fn import_file_emit_errors() {
    let prelude = "Op :: std.Enum[\"one\"]\nnum :: std.Type[]\n";
    let build = |body: &str| {
        import_file(
            "emit.qxc",
            &format!(
                "#builtin.build .= () => std.Folder: [\n  \"x.txt\" .= std.File: {{\n{body}\n    -> \"x\"\n  }}\n]\n{prelude}std :: #builtin.std"
            ),
        )
        .map(|_| ())
    };
    let message = |result: Result<(), Vec<TokenizationError>>| {
        let errors = result.expect_err("expected the build to fail");
        assert_eq!(errors.len(), 1, "{errors:?}");
        errors[0].entries[0].message.clone()
    };

    assert_eq!(
        message(build("    _ = std.emit(num, Op.one, ())")),
        "std.emit of Op can't run at compile time"
    );
    assert_eq!(
        message(build(
            "    m := std.kw.mut(Op).new: Op.one\n    _ = std.emit(num, m.*, ())"
        )),
        "std.emit's data must be known at compile time"
    );
    assert_eq!(
        message(build("    _ = std.emit(num, Op.one, 1)")),
        "std.emit's operands are a list in parentheses, as in (a, b)"
    );
}
