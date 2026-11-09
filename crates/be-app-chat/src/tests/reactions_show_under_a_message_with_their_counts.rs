use super::*;

#[test]
fn reactions_show_under_a_message_with_their_counts() {
    let mut chat = Harness::new(&[
        (ROSA, "The release is out!"),
        (ME, "Thanks everyone"),
    ]);
    let (release, thanks) = (chat.id(0), chat.id(1));
    let other = Uuid::from_u128(0x3b0c_0000_0000_4000_8000_0000_0000_0004);
    for (author, reaction) in [(ROSA, "party-popper"), (other, "party-popper"), (other, "heart")] {
        chat.arrive(ChatContent::react(release, author, reaction, MORNING));
    }
    chat.click(&format!("chat.message.{release}.reaction.heart"));
    chat.click(&format!("chat.message.{thanks}.react"));

    assert_eq!(
        chat.content().reactions(release),
        [
            ("heart".to_owned(), vec![other, ME]),
            ("party-popper".to_owned(), vec![other, ROSA]),
        ]
    );
    assert_eq!(
        chat.editor
            .label(&format!("chat.message.{release}.reaction.heart")),
        "\u{e0f2} 2",
        "the heart's icon and its count"
    );
    assert!(
        chat.editor
            .shown(&format!("chat.message.{thanks}.react.thumbs-up")),
        "the reactions to choose from open under the message"
    );
    chat.snapshot("reactions_show_under_a_message_with_their_counts");
}
