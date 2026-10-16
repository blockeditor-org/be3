use super::*;

#[test]
fn an_invite_code_opens_only_its_own_workspaces_key() {
    let code = InviteCode::generate();
    let typed = code.to_string().to_lowercase().replace('-', " ");
    let parsed = InviteCode::parse(&typed).expect("a printed code parses back");
    assert!(
        parsed == code,
        "the code changed on the way through its text"
    );

    let sealed = code.seal(b"workspace", b"the workspace key");
    assert_eq!(
        parsed.open(b"workspace", &sealed).unwrap(),
        b"the workspace key"
    );
    assert_eq!(
        parsed.open(b"another workspace", &sealed),
        Err(KeyError::Invite),
        "the key opened for a workspace it was not sealed for"
    );
    assert_eq!(
        InviteCode::generate().open(b"workspace", &sealed),
        Err(KeyError::Invite),
        "another code opened the key"
    );
    assert_eq!(InviteCode::parse("ABCD-EFGH"), Err(KeyError::Invite));
}
