use super::*;

#[test]
fn import_file_written_calls_match_calls() {
    let call = build_c_fn("  -> g(a)").unwrap_or_else(|errors| panic!("{errors:?}"));
    let written =
        build_c_fn("  -> g.[std.operator.call](a)").unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(call, written);

    assert_eq!(
        only_error(build_c_fn("  -> a.[std.operator.call](b)")),
        "int has no std.operator.call"
    );
}
