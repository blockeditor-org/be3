use super::*;

#[test]
fn text_that_names_no_key_says_which_rule() {
    let error = text::parse(
        r#"
        [[group]]
        name = "Cairn"
        rules = [{ on = "Q", send = "E" }, { on = "Q", tap = "Spcae" }]
        "#,
    )
    .expect_err("Spcae is not a key");
    assert_eq!(error, r#"Cairn, rule 2: no key or button is named "Spcae""#);

    let error = text::parse(
        r#"
        [[group]]
        rules = [{ on = "Q", send = "E", tap = "E" }]
        "#,
    )
    .expect_err("a rule does one thing");
    assert_eq!(
        error,
        "Group 1, rule 1: it has more than one of send, tap and steps"
    );

    assert!(text::parse("[[group]]\nrules = [{ on = \"Q\", sned = \"E\" }]").is_err());
}
