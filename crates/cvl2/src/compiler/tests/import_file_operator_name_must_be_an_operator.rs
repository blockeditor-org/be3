use super::*;

#[test]
fn import_file_operator_name_must_be_an_operator() {
    for name in ["++", "::", ","] {
        assert_eq!(
            only_error(build_c_fn(&format!(
                "  -> std.c.int.[std.operator.slot(\"{name}\")](a, b)"
            ))),
            format!("not an operator: \"{name}\"")
        );
    }
}
