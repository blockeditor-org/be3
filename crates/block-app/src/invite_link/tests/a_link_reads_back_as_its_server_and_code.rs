use super::*;

#[test]
fn a_link_reads_back_as_its_server_and_code() {
    let code = InviteCode::generate();
    let text = link("https://notes.example.com/", &code);
    assert!(
        text.starts_with("https://notes.example.com/#invite/"),
        "{text}"
    );

    let parsed = InviteLink::parse(&format!("  {text}\n")).expect("the link parses");
    assert!(
        parsed.code == code,
        "the code changed on the way through the link"
    );
    assert_eq!(parsed.server.as_deref(), Some("https://notes.example.com"));
    assert!(parsed.is_for("https://notes.example.com"));
    assert!(!parsed.is_for("https://elsewhere.example.com"));

    let typed = InviteLink::parse(&code.to_string().to_lowercase()).expect("a bare code parses");
    assert!(typed.code == code && typed.server.is_none());
    assert!(typed.is_for("https://anywhere.example.com"));
    assert_eq!(
        InviteLink::parse("https://notes.example.com/#invite/nope"),
        None
    );
}
