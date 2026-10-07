use super::*;

fn build_with_types(body: &str) -> Result<String, Vec<TokenizationError>> {
    import_file(
        "types.qxc",
        &format!(
            "#builtin.build .= () => std.Folder: [
  \"x\" .= std.File: {{
{body}
    -> \"x\"
  }}
]
M :: std.Type: [
  std.type.repr .= std.kw.int
]
N :: std.Type: [ ]
L :: std.Type: [
  L.[std.literal.number] .= 1
]
std :: #builtin.std"
        ),
    )
    .map(|_| String::new())
}

#[test]
fn import_file_user_type_errors() {
    let cases = [
        (
            "    x := std.type.wrap(M, std.kw.bool.true)",
            "expected KwInt, got KwBool",
        ),
        (
            "    x := std.type.unwrap(std.kw.int: 1)",
            "std.type.unwrap needs a value of a std.Type, got KwInt",
        ),
        ("    x := std.type.wrap(N, 1)", "N has no std.type.repr"),
        (
            "    x := L: 1",
            "dependency loop while reading the keys of L",
        ),
        ("    x := M: 1", "Number is not supported in slot: M"),
        (
            "    x := std.type.wrap(M, 1) + 1",
            "operator + is not supported: TypeUnknown has no std.operator.slot(\"+\") and M has no std.operator.lhs(\"+\")",
        ),
    ];
    for (body, message) in cases {
        assert_eq!(only_error(build_with_types(body)), message, "{body}");
    }
}
