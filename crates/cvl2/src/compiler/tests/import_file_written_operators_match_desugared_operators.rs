use super::*;

#[test]
fn import_file_written_operators_match_desugared_operators() {
    let pairs = [
        (
            "  -> a + b",
            "  -> std.c.int.[std.operator.slot(\"+\")](a, b)",
        ),
        ("  -> a == b", "  -> a.[std.operator.lhs(\"==\")](b)"),
        (
            "  x := a * b\n  -> x",
            "  x := a.[std.operator.lhs(\"*\")](b)\n  -> x",
        ),
    ];
    for (operator, written) in pairs {
        let operator = build_c_fn(operator).unwrap_or_else(|errors| panic!("{errors:?}"));
        let written = build_c_fn(written).unwrap_or_else(|errors| panic!("{errors:?}"));
        assert_eq!(operator, written);
    }

    assert_eq!(
        only_error(build_c_fn(
            "  -> std.c.int.[std.operator.slot(\"==\")](a, b)"
        )),
        "int has no std.operator.slot(\"==\")"
    );
}
