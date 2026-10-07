use super::*;

fn build(cond: &str) -> Result<ComptimeValueBuildArtifact, Vec<TokenizationError>> {
    import_file(
        "op.qxc",
        &format!(
            "#builtin.build .= () => std.Folder: [
  \"lib.c\" .= std.c.compile: [
    \"f\" .= f
  ]
]
f :: (a: std.c.int) => std.c.int: {{
  std.c.if ({cond}) {{ }}
  -> 0
}}
std :: #builtin.std"
        ),
    )
}

#[test]
fn import_file_compare_operator_takes_rhs_type_from_lhs() {
    build("a == 1").unwrap_or_else(|errors| panic!("a == 1 failed: {errors:?}"));

    let errors = build("1 == a").expect_err("the lhs literal has no type to use");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].entries[0].message,
        "Number is not supported in slot: TypeUnknown"
    );
}
