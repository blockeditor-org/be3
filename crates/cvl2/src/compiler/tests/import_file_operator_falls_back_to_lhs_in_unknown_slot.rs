use super::*;

#[test]
fn import_file_operator_falls_back_to_lhs_in_unknown_slot() {
    let source = build_c_fn("  x := a + 1\n  -> x").unwrap_or_else(|errors| panic!("{errors:?}"));

    assert!(source.contains("int _3 = _a0 + 1;"), "{source}");
}
