use super::*;

fn build_with_types(body: &str) -> Result<String, Vec<TokenizationError>> {
    import_file(
        "data.qxc",
        &format!(
            "#builtin.build .= () => std.Folder[
  \"x\" .= std.File: {{
{body}
    -> \"x\"
  }}
]
Point :: std.Struct[\"x\" .= std.kw.int, \"y\" .= std.kw.int]
Shape :: std.Enum[\"circle\" .= std.kw.int, \"dot\"]
Bogus :: std.Type[.bogus .= []]
Both :: std.Type[.fields .= [\"x\" .= std.kw.int], .cases .= [\"a\"]]
Twice :: std.Enum[\"a\", \"a\"]
Untyped :: std.Struct[\"x\"]
Pt :: std.Type[
  .fields .= [\"x\" .= std.kw.int]
  .statics .= [\"make\" .= () => Pt[\"x\" .= 1]]
  .methods .= [\"get\" .= (p: Pt) => std.kw.int: p.x]
]
std :: #builtin.std"
        ),
    )
    .map(|_| String::new())
}

#[test]
fn import_file_struct_and_enum_errors() {
    let cases = [
        (
            "    p := Point[\"x\" .= 1]",
            "missing field \"y\" for Point",
        ),
        (
            "    p := Point[\"x\" .= 1, \"y\" .= 2, \"z\" .= 3]",
            "Point has no field \"z\"",
        ),
        (
            "    p := Point[\"x\" .= 1, \"y\" .= 2]\n    z := p.z",
            "Point has no field 'z'",
        ),
        (
            "    s := Shape: .circle(\"r\")",
            "String is not supported in slot: KwInt",
        ),
        ("    s := Shape: .square", "Shape has no field 'square'"),
        (
            "    b := Bogus: 1",
            "std.Type entries must be .fields, .cases, .statics or .methods",
        ),
        (
            "    b := Both: 1",
            "a std.Type can't have both .fields and .cases",
        ),
        ("    t := Twice: .a", "duplicate case \"a\""),
        (
            "    u := Untyped[\"x\" .= 1]",
            "field \"x\" needs a type, as in \"x\" .= T",
        ),
        ("    g := Pt.get", "Pt has no field 'get'"),
        (
            "    p := Pt.make()\n    m := p.make",
            "Pt has no field 'make'",
        ),
    ];
    for (body, message) in cases {
        assert_eq!(only_error(build_with_types(body)), message, "{body}");
    }

    assert_eq!(
        build_with_types(
            "    s := Shape: .circle(2)\n    r := s.circle.?\n    d := s.dot\n    p := Point[\"x\" .= r, \"y\" .= 2]\n    v := Pt.make().get()"
        ),
        Ok(String::new())
    );
}
