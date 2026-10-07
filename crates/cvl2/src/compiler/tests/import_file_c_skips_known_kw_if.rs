use super::*;

#[test]
fn import_file_c_skips_known_kw_if() {
    assert_eq!(
        build_c_fn(
            "  -> std.c.int: :out {\n    std.kw.if (.false) {\n      _ = not_defined\n    } .else_if (.true) {\n      out: b\n    } .else {\n      _ = also_not_defined\n    }\n  }"
        )
        .unwrap_or_else(|errors| panic!("{errors:?}")),
        "int f(int _a0, int _a1);\n\nint f(int _a0, int _a1) {\n    int _0;\n    {\n        _0 = _a1;\n        goto _l0;\n    }\n    _l0:;\n    return _0;\n}\n"
    );
    assert_eq!(
        only_error(build_c_fn(
            "  x := std.kw.mut(std.kw.bool).new: .true\n  -> a"
        )),
        "std.kw.mut is not supported in C"
    );
}
