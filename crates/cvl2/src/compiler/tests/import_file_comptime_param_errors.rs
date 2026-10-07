use super::*;

#[test]
fn import_file_comptime_param_errors() {
    let helpers = "scale :: (k :: std.kw.int, x: std.c.int) => std.c.int: x * std.c.int.[std.literal.number](k)\nlabel :: (t :: std.kw.text) => std.c.int: 0\nforever :: (n :: std.kw.int) => std.kw.int: forever(n + 1)\ntwice :: (k :: std.kw.int) => std.kw.int: k * 2";
    let c = |body: &str| {
        build_c_fn(&format!(
            "  -> {body}\n}}\n{helpers}\nunused :: () => std.c.int: {{\n  -> 0"
        ))
    };

    let file = |body: &str| {
        build_file(&format!(
            "{body}\n    -> \"x\"\n  }}\n]\n{helpers}\nunused :: [\n  {{"
        ))
    };
    assert_eq!(
        only_error(file(
            "    m := std.kw.mut(std.kw.int: 2)\n    _ = twice(m.*)"
        )),
        "k must be known at compile time"
    );
    assert_eq!(only_error(c("scale(2)")), "expected 2 arguments, got 1");
    assert_eq!(
        only_error(c("label(\"x\")")),
        "a KwText can't be a compile-time argument"
    );
    assert_eq!(
        only_error(file("    _ = forever(0)")),
        "this function was copied for 256 sets of compile-time arguments; does a recursive call change one every time?"
    );

    let errors = import_file(
        "export.qxc",
        &format!("#builtin.build .= () => std.Folder: [\n  \"lib.c\" .= std.c.compile: [\n    \"scale\" .= scale\n  ]\n]\n{helpers}\nstd :: #builtin.std"),
    )
    .expect_err("an export can't have compile-time parameters");
    assert_eq!(
        errors[0].entries[0].message,
        "a function with compile-time parameters can only be called, which gives them values"
    );
}
