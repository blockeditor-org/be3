use super::*;

#[test]
fn a_handoff_without_the_token_is_refused() {
    let asked = Handoff {
        dev_workspace: true,
    };
    assert_eq!(parse(&request("secret", asked), "secret"), Ok(asked));
    assert!(parse(&request("guess", asked), "secret").is_err());
    assert!(parse("secret quit", "secret").is_err());
    assert!(parse("secret show everything", "secret").is_err());
}
