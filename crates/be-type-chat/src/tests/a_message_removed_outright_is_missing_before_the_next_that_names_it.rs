use super::*;

fn removed(chat: &ChatContent, message: ObjectId) -> ChatContent {
    edited(chat, [[be_block_model::Change::remove(message)].into_iter().collect()])
}

/// A message removed outright, not deleted, is missing before the first later message that
/// names it: as the last one its author saw, or as its author's own message before it, which
/// tells even when another's message came between. A deletion is no gap.
#[test]
fn a_message_removed_outright_is_missing_before_the_next_that_names_it() {
    let chat = sent(&ChatContent::default(), ADA, &["one", "two", "three"]);
    let [two, three] = [chat.messages()[1], chat.messages()[2]];
    assert_eq!(chat.missing_before(), [], "nothing is missing");
    assert_eq!(
        removed(&chat, two).missing_before(),
        [(three, two)],
        "the next message names it"
    );
    assert_eq!(
        edited(&chat, [chat.delete(two, ADA, 9)]).missing_before(),
        [],
        "a deletion is no gap"
    );

    // Bo's message came between Ada's two, so Ada's second saw Bo's, and names her own first
    // only as her own.
    let ada = sent(&ChatContent::default(), ADA, &["mine"]);
    let mine = ada.messages()[0];
    let bo = ChatContent::send_as(
        ObjectId::new(),
        &Message::new(BO, 1, "his, written before hers arrived"),
    );
    let both = edited(&ada, [bo]);
    let his = both.messages()[1];
    let next = sent(&both, ADA, &["mine again"]);
    let again = next.messages()[2];
    let sent_again = next.message(again).expect("the message was sent");
    assert_eq!(sent_again.seen.map(|seen| seen.id), Some(his));
    assert_eq!(sent_again.own_previous.map(|own| own.id), Some(mine));
    assert_eq!(removed(&next, mine).missing_before(), [(again, mine)]);
}
