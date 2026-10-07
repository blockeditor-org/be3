use super::*;

#[test]
fn import_file_bad_token_reports_once() {
    let errors = build_file("    -> \"x\" ^^ \"y\"").expect_err("^^ is not a token");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].entries[0].message, "bad token \"^^\"");
}
