use super::*;

#[test]
fn a_contact_link_is_not_an_invite_link() {
    let code = InviteCode::generate();
    let text = contact_link("https://notes.example.com", &code);
    assert!(
        text.starts_with("https://notes.example.com/#contact/"),
        "{text}"
    );
    let parsed = InviteLink::parse_contact(&text).expect("the contact link parses");
    assert!(parsed.code == code);
    assert!(parsed.is_for("https://notes.example.com"));
    assert_eq!(InviteLink::parse(&text), None);
    assert_eq!(
        InviteLink::parse_contact(&link("https://notes.example.com", &code)),
        None
    );
}
