use super::*;

#[test]
fn import_file_text_interpolates_and_names_fresh_identifiers() {
    let text = "    a := std.kw.text.fresh(\"v\")
    b := std.kw.text.fresh(\"v\")
    c := std.kw.text.fresh(\"w\")
    t := std.kw.text: \"\\(a) \\(b) \\(a) \\(c) \\(std.kw.int: 7)\"
    -> t.render";
    assert_eq!(build_file(text), Ok("v_0 v_1 v_0 w_0 7".to_string()));

    let string = "    name := std.kw.string: \"world\"
    -> std.kw.string: \"hello, \\(name)!\"";
    assert_eq!(build_file(string), Ok("hello, world!".to_string()));

    assert_eq!(
        only_error(build_file(
            "    -> std.kw.string: \"n = \\(std.kw.int: 1)\""
        )),
        "expected KwString, got KwInt"
    );
}
