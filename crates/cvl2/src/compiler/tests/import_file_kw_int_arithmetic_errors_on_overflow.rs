use super::*;

#[test]
fn import_file_kw_int_arithmetic_errors_on_overflow() {
    assert_eq!(
        only_error(build_file("    x := std.kw.int: 1 / 0\n    -> \"x\"")),
        "1 / 0 overflows or divides by zero"
    );
    assert_eq!(
        only_error(build_file(
            "    x := std.kw.int: 9223372036854775807 + 1\n    -> \"x\""
        )),
        "9223372036854775807 + 1 overflows or divides by zero"
    );
}
