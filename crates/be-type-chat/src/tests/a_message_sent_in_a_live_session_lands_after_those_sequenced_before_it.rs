use be_block_model::Touched;

use super::*;

#[test]
fn a_message_sent_in_a_live_session_lands_after_those_sequenced_before_it() {
    let confirmed = sent(&ChatContent::default(), ADA, &["hello"]);
    let (_, mine) = confirmed.send(CHAT, Message::new(BO, 5, "mine, still unconfirmed"));
    let (_, earlier) = confirmed.send(CHAT, Message::new(ADA, 4, "sequenced first"));
    let mut visible = edited(&confirmed, [mine.clone()]);
    let mut confirmed = confirmed;

    // The owner sequences another member's message before ours reaches it.
    visible.apply_beneath(
        &mut confirmed,
        &earlier.into(),
        &[mine.into()],
        &mut Vec::<Touched>::new(),
    );

    assert_eq!(
        bodies(&visible),
        ["hello", "sequenced first", "mine, still unconfirmed"]
    );
}
