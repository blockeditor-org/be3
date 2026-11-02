use super::*;

#[test]
fn an_invite_code_opens_only_its_own_organizations_key() {
    let code = InviteCode::generate();
    let typed = code.to_string().to_lowercase().replace('-', " ");
    let parsed = InviteCode::parse(&typed).expect("a printed code parses back");
    assert!(
        parsed == code,
        "the code changed on the way through its text"
    );

    let sealed = code.seal(b"organization", b"the organization key");
    assert_eq!(
        parsed.open(b"organization", &sealed).unwrap(),
        b"the organization key"
    );
    assert_eq!(
        parsed.open(b"another organization", &sealed),
        Err(KeyError::Invite),
        "the key opened for an organization it was not sealed for"
    );
    assert_eq!(
        InviteCode::generate().open(b"organization", &sealed),
        Err(KeyError::Invite),
        "another code opened the key"
    );
    assert_eq!(InviteCode::parse("ABCD-EFGH"), Err(KeyError::Invite));
}
