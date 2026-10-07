use super::*;

fn pick(body: &str) -> Result<String, Vec<TokenizationError>> {
    let artifact = import_file(
        "bind.qxc",
        &format!(
            "#builtin.build .= () => std.Folder[
  \"x\" .= std.File: f(std.Option(std.kw.int): 3)
]
f :: (o: std.Option(std.kw.int)) => std.kw.string: :out {{
{body}
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
fn import_file_kw_if_binds_an_optional() {
    let cases = [
        (
            "  std.kw.if (v: std.kw.int := o) { out: std.kw.string.from_int(v + 1) }\n  -> \"none\"",
            "4",
        ),
        (
            "  std.kw.if (_ := o) { out: \"some\" }\n  -> \"none\"",
            "some",
        ),
        (
            "  std.kw.if (o): (v) => { out: std.kw.string.from_int(v) }\n  -> \"none\"",
            "3",
        ),
        (
            "  std.kw.if (v := o) {\n    out: \"some\"\n  } .else {\n    out: \"none\"\n  }",
            "some",
        ),
    ];
    for (body, expected) in cases {
        assert_eq!(pick(body), Ok(expected.to_string()), "{body}");
    }

    let errors = [
        (
            "  std.kw.if (v := std.kw.int: 1) { out: \"a\" }\n  -> \"none\"",
            "std.kw.if (v := x) needs an optional, got KwInt",
        ),
        (
            "  std.kw.if (v := o) { out: \"a\" } .else { }",
            "expected KwString, got TypeVoid",
        ),
        (
            "  std.kw.if (v: std.kw.string := o) { out: v }\n  -> \"none\"",
            "expected KwString, got KwInt",
        ),
    ];
    for (body, message) in errors {
        let got = pick(body).expect_err(body);
        assert_eq!(got[0].entries[0].message, message, "{body}");
    }
}
