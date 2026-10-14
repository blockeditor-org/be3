use super::*;

#[test]
fn import_file_compound_assignment() {
    let built = build_file(
        "    n := std.kw.mut(std.kw.int).new: 10
    n += 5
    n -= 3
    n *= 4
    n /= 6
    n %= 5
    s := std.kw.mut(std.kw.string).new: \"a\"
    s += \"bc\"
    -> s.* + std.kw.string.from_int(n.*)",
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(built, "abc3");

    assert_eq!(
        only_error(build_file(
            "    b := std.kw.mut(std.kw.bool).new: .true\n    b += .false\n    -> \"x\""
        )),
        "operator += is not supported: KwBool has no std.operator.lhs(\"+\")"
    );

    let c = build_c_fn("  x := std.kw.mut(std.c.int).new: a\n  -> a");
    assert!(c.is_err(), "C has no cells");
}
