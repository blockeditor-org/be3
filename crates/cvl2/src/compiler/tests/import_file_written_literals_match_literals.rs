use super::*;

#[test]
fn import_file_written_literals_match_literals() {
    let literal = build_c_fn("  -> 5").unwrap_or_else(|errors| panic!("{errors:?}"));
    let written = build_c_fn("  -> std.c.int.[std.literal.number]: 5")
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(literal, written);

    assert_eq!(
        only_error(build_c_fn("  -> std.c.int.[std.literal.number]: \"x\"")),
        "expected a number literal"
    );
    assert_eq!(
        only_error(build_c_fn("  -> std.c.int.[std.literal.string]: \"x\"")),
        "CInt has no std.literal.string"
    );
}
