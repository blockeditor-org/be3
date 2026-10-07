use super::*;

fn classify(n: i64) -> Result<String, Vec<TokenizationError>> {
    let n = if n < 0 {
        format!("0 - {}", -n)
    } else {
        n.to_string()
    };
    let artifact = import_file(
        "chain.qxc",
        &format!(
            "#builtin.build .= () => std.Folder[
  \"x\" .= std.File: describe(std.kw.mut(std.kw.int).new({n}).*)
]
describe :: (n: std.kw.int) => std.kw.string: :out {{
  std.kw.if (n < 0) {{
    out: \"negative\"
  }} .else_if (n == 0) {{
    out: \"zero\"
  }} .else_if (n < 10) {{
    out: \"small\"
  }} .else {{
    out: \"big\"
  }}
}}
std :: #builtin.std"
        ),
    )?;
    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let [(_, ComptimeValueBuildArtifact::File(file))] = folder.value.as_slice() else {
        panic!("expected a single file");
    };
    Ok(String::from_utf8(file.value.clone()).unwrap())
}

#[test]
fn import_file_kw_else_if_chains() {
    for (n, expected) in [(-4, "negative"), (0, "zero"), (7, "small"), (12, "big")] {
        assert_eq!(classify(n), Ok(expected.to_string()), "{n}");
    }

    assert_eq!(
        only_error(build_file("    .else_if (.true) { }\n    -> \"x\"")),
        ".else_if must follow the } of a std.kw.if on the same line"
    );
}
