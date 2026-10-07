use super::*;

fn pick(cond: &str) -> Result<String, Vec<TokenizationError>> {
    build_file(&format!(
        "    -> std.File: :out {{\n      std.kw.if ({cond}) {{ out: \"yes\" }}\n      -> \"no\"\n    }}"
    ))
}

#[test]
fn import_file_kw_bool_values_resolve_from_slot() {
    assert_eq!(pick(".true"), Ok("yes".to_string()));
    assert_eq!(pick(".false"), Ok("no".to_string()));
    assert_eq!(pick("std.kw.bool.true"), Ok("yes".to_string()));
    assert_eq!(pick("(std.kw.bool: .false) == .true"), Ok("no".to_string()));
    assert_eq!(
        pick("(std.kw.bool: .false) != .true"),
        Ok("yes".to_string())
    );

    let errors = pick(".maybe").expect_err(".maybe is not a bool");
    assert_eq!(errors[0].entries[0].message, "KwBool has no field 'maybe'");
}
