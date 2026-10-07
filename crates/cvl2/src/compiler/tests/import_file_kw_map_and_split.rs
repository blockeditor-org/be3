use super::*;

#[test]
fn import_file_kw_map_and_split() {
    let built = build_file(
        "    m := std.kw.map(std.kw.string, std.kw.int): [\"a\" .= 1, \"b\" .= 2]
    n := m.set(\"a\", 5).set(\"c\", 3)
    parts := std.kw.string: \"x:y:z\"
    -> std.kw.string.from_int(n.get(\"a\").?) + \" \" + n.keys.join(\",\") + \" \" + std.kw.string.from_int(n.len) + \" \" + std.kw.string.from_int(m.get(\"a\").?) + \" \" + parts.split(\":\").join(\"/\")",
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(built, "5 a,b,c 3 1 x/y/z");

    assert_eq!(
        only_error(build_file(
            "    m := std.kw.map(std.kw.text, std.kw.int): [std.kw.text.fresh(\"t\") .= 1]\n    -> \"x\""
        )),
        "this value can't be a std.kw.map key"
    );
}
