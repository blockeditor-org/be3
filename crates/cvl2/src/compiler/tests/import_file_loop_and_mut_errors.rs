use super::*;

fn build_with_helpers(body: &str) -> Result<String, Vec<TokenizationError>> {
    import_file_with_step_limit(
        "loop.qxc",
        &format!(
            "#builtin.build .= () => std.Folder: [
  \"x\" .= std.File: {{
{body}
    -> \"x\"
  }}
]
forever :: (n: std.kw.int) => std.kw.int: forever(n)
early :: (n: std.kw.int) => {{
  -> early(n)
}}
spin :: () => std.kw.int: :out {{
  std.kw.loop {{ }}
}}
std :: #builtin.std"
        ),
        10_000,
    )
    .map(|_| String::new())
}

#[test]
fn import_file_loop_and_mut_errors() {
    let cases = [
        (
            "    y := spin()",
            "compile-time evaluation took too many steps; is a std.kw.loop missing a break?",
        ),
        (
            "    y := forever(1)",
            "compile-time calls are nested too deeply",
        ),
        (
            "    x := std.kw.int: 1\n    x = 5",
            "operator = is not supported: TypeVoid has no std.operator.slot(\"=\") and KwInt has no std.operator.lhs(\"=\")",
        ),
        (
            "    x := std.kw.int: 1\n    y := x.*",
            "KwInt has no field '*'",
        ),
        (
            "    m := std.kw.mut(std.kw.int).new: 1\n    m = std.kw.bool.true",
            "expected KwInt, got KwBool",
        ),
        (
            "    x := std.kw.int: :l {\n      l: 1\n      -> 2\n    }",
            "extra lines not allowed after return",
        ),
    ];
    for (body, message) in cases {
        assert_eq!(only_error(build_with_helpers(body)), message, "{body}");
    }

    assert_eq!(
        only_error(build_c_fn("  x := std.kw.mut(std.kw.int).new: 1\n  -> a")),
        "std.kw.mut is not supported in C"
    );
}
