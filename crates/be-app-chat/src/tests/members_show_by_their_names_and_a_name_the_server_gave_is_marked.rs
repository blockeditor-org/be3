use super::*;

#[test]
fn members_show_by_their_names_and_a_name_the_server_gave_is_marked() {
    const TEO: Uuid = Uuid::from_u128(0x7e0a_0000_0000_4000_8000_0000_0000_0004);
    let mut chat = Harness::trusting(
        &[
            (ROSA, "Morning! Is the release still on for today?"),
            (TEO, "I can check the installers."),
            (ME, "It is. Thanks, both."),
        ],
        &[(TEO, Signer::Unverified)],
    );
    let (rosa, teo, mine) = (chat.id(0), chat.id(1), chat.id(2));
    // Until the host names them, members show by their account's first digits.
    assert_eq!(
        chat.row(rosa),
        "Member 7e1a 09:00 Morning! Is the release still on for today?"
    );

    // Rosa is a contact, named by her card; Teo is not, and is named by what the server says,
    // which the row marks.
    chat.editor.set_members(vec![
        be_sdk_beui::BlockMember {
            account: ME,
            name: "Sam Okafor".to_owned(),
            verified: true,
        },
        be_sdk_beui::BlockMember {
            account: ROSA,
            name: "Rosa Diaz".to_owned(),
            verified: true,
        },
        be_sdk_beui::BlockMember {
            account: TEO,
            name: "Teo Lind".to_owned(),
            verified: false,
        },
    ]);
    chat.run();
    assert_eq!(
        chat.row(rosa),
        "Rosa Diaz 09:00 Morning! Is the release still on for today?"
    );
    assert_eq!(
        chat.row(teo),
        "Teo Lind unverified name 09:03 I can check the installers.",
        "a name the server gave carries one caption, for the key not verified too"
    );
    assert_eq!(chat.row(mine), "You 09:06 It is. Thanks, both.");
    assert!(!chat.editor.shown(&format!("chat.message.{rosa}.name_unverified")));
    assert!(chat.editor.shown(&format!("chat.message.{teo}.name_unverified")));

    // A reply quotes its message's author by name too, and a name the server gave says so in
    // the quote, as in the composer's "Replying to", where no caption follows it.
    chat.click(&format!("chat.message.{teo}.reply"));
    chat.send("Great, thank you.");
    let reply = chat.id(3);
    assert!(
        chat.row(reply)
            .contains("Teo Lind (unverified name): I can check the installers."),
        "{}",
        chat.row(reply)
    );
    chat.click(&format!("chat.message.{rosa}.reply"));
    assert!(
        chat.editor
            .label("chat.replying")
            .contains("Replying to Rosa Diaz: Morning!"),
        "a contact's name is quoted as it is: {}",
        chat.editor.label("chat.replying")
    );
    chat.click(&format!("chat.message.{teo}.reply"));
    assert!(
        chat.editor
            .label("chat.replying")
            .contains("Replying to Teo Lind (unverified name): I can check the installers."),
        "{}",
        chat.editor.label("chat.replying")
    );
    chat.snapshot("members_show_by_their_names_and_a_name_the_server_gave_is_marked");
}
