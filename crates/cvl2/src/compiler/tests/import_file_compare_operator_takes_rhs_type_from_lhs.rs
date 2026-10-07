use super::*;

#[test]
fn import_file_compare_operator_takes_rhs_type_from_lhs() {
    build_c_fn("  -> a == 1").unwrap_or_else(|errors| panic!("a == 1 failed: {errors:?}"));

    assert_eq!(
        only_error(build_c_fn("  -> 1 == a")),
        "Number is not supported in slot: TypeUnknown"
    );
}
