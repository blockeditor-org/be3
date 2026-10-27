use super::*;

#[test]
fn an_unclosed_quote_in_a_command_is_an_error() {
    let error = commands("wait 'the day's cell'").unwrap_err();
    assert!(error.contains("unclosed quote"), "{error}");
}
