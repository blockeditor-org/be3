use super::*;
use be_type_chat::MessageEdit;

#[test]
fn a_message_shows_whether_its_author_signed_it() {
    let sam = Uuid::from_u128(0x5a30_0000_0000_4000_8000_0000_0000_0005);
    let kai = Uuid::from_u128(0x4a10_0000_0000_4000_8000_0000_0000_0006);
    let mut chat = Harness::trusting(
        &[
            (ROSA, "Signed by a contact"),
            (sam, "Signed by a member who is not a contact"),
            (kai, "Signed with a key the server changed"),
            (ME, "Signed by me"),
        ],
        &[(sam, Signer::Unverified), (kai, Signer::Changed)],
    );
    let forged = signed_by(
        &chat.content(),
        sam,
        Message::new(ROSA, MORNING + 20 * MINUTE, "Rosa never wrote this"),
    );
    chat.arrive(forged);
    let rows: Vec<String> = (0..5).map(|index| chat.row(chat.id(index))).collect();
    assert_eq!(
        rows,
        [
            "Member 7e1a 09:00 Signed by a contact",
            "Member 5a30 09:03 not verified Signed by a member who is not a contact",
            "Member 4a10 09:06 key changed Signed with a key the server changed",
            "You 09:09 Signed by me",
            "Member 7e1a 09:20 forged Rosa never wrote this",
        ]
    );
    // Someone with Edit rewrites Rosa's message in her name.
    let rewrite = MessageEdit {
        author: ROSA,
        time: MORNING + 25 * MINUTE,
        body: "Rewritten by someone else".into(),
        signature: Signature(be_sdk_test::signature(sam, b"not Rosa's edit")),
    };
    let first = chat.id(0);
    chat.arrive(chat.content().put_edit(first, &rewrite));
    assert!(
        chat.row(first).contains("forged"),
        "a message whose edit its author did not sign is forged: {}",
        chat.row(first)
    );
    chat.snapshot("a_message_shows_whether_its_author_signed_it");
}
