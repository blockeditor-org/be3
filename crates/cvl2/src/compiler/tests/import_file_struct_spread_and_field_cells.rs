use super::*;

fn build_with_point(body: &str) -> Result<String, Vec<TokenizationError>> {
    build_file(&format!(
        "{body}
  }}
]
s :: (n: std.kw.int) => std.kw.string: std.kw.string.from_int(n)
Point :: std.Struct[\"x\" .= std.kw.int, \"y\" .= std.kw.int]
Line :: std.Struct[\"from\" .= Point, \"to\" .= Point]
unused :: [
  {{"
    ))
}

#[test]
fn import_file_struct_spread_and_field_cells() {
    let built = build_with_point(
        "    p := Point[\"x\" .= 1, \"y\" .= 2]
    q := Point[\"y\" .= 5, ...p]
    c := std.kw.mut(Point).new: q
    before := c.*
    c.x = 7
    c.y = c.y.* + c.x.*
    l := std.kw.mut(Line).new: Line[\"from\" .= p, \"to\" .= q]
    l.to.x = 9
    -> s(c.*.x) + \",\" + s(c.y.*) + \",\" + s(p.x) + \",\" + s(q.y) + \",\" + s(before.y) + \",\" + s(l.to.x.*) + \",\" + s(l.*.from.x)",
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(built, "7,12,1,5,5,9,1");

    assert_eq!(
        only_error(build_with_point(
            "    p := Point[\"x\" .= 1, \"x\" .= 2]\n    -> \"x\""
        )),
        "field \"x\" is given twice"
    );
    assert_eq!(
        only_error(build_with_point("    p := Point[...3]\n    -> \"x\"")),
        "Number is not supported in slot: Point"
    );
    assert_eq!(
        only_error(build_with_point("    p := Point[\"x\" .= 1]\n    -> \"x\"")),
        "missing field \"y\" for Point"
    );
}
