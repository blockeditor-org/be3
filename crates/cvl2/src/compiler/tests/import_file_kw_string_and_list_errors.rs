use super::*;

#[test]
fn import_file_kw_string_and_list_errors() {
    let cases = [
        (
            "    x := (std.kw.list(std.kw.string): (\"a\")).get(1)",
            "index 1 is out of range for a list of length 1",
        ),
        (
            "    x := (std.kw.list(std.kw.int): (1, 2)).join(\",\")",
            "KwList has no field 'join'",
        ),
        (
            "    x := (std.kw.list(std.kw.int): (1)).push(\"a\")",
            "String is not supported in slot: KwInt",
        ),
        (
            "    x := std.kw.string: \"a\" + 1",
            "Number is not supported in slot: KwString",
        ),
        (
            "    x := std.kw.string: \"a\" - \"b\"",
            "String is not supported in slot: TypeUnknown",
        ),
    ];
    for (body, message) in cases {
        let message = message.to_string();
        let got = only_error(build_file(&format!("{body}\n    -> \"x\"")));
        assert!(got.starts_with(&message), "{body}: {got}");
    }

    assert_eq!(
        build_file(
            "    -> std.File: :out {\n      std.kw.if ((std.kw.string: \"ab\") == \"a\" + \"b\") { out: \"same\" }\n      -> \"different\"\n    }"
        ),
        Ok("same".to_string())
    );
}
