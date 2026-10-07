use super::*;

#[test]
fn import_file_operator_errors_without_slot_or_lhs() {
    assert_eq!(
        only_error(build_c_fn("  x := f + 1\n  -> 0")),
        "operator + is not supported: TypeUnknown has no std.operator.slot(\"+\") and TypeFn has no std.operator.lhs(\"+\")"
    );
}
