use super::*;
use crate::store::mailbox::{MAX_LETTERS_FROM_ONE, leave};

/// A new phrase must never wait on a contact that does not read its mailbox: past the bound on
/// one sender's key changes, the oldest goes and the new one is kept. Accepted cards are refused
/// past theirs instead, and other senders and the server's notices are not held back.
#[tokio::test]
async fn a_key_change_past_the_bound_lets_the_oldest_go_but_an_accepted_card_is_refused() {
    let harness = Harness::start().await;
    let to = harness.member("to@example.com").await.id();
    let from = harness.member("from@example.com").await.id();
    let other = harness.member("other@example.com").await.id();
    let database = metadata_database(&harness.directory);
    for change in 0..=MAX_LETTERS_FROM_ONE {
        leave(
            &database,
            to,
            Some(from),
            &be_block_protocol::Letter::KeyChange(vec![change as u8]),
        )
        .unwrap();
    }
    let card = |link| be_block_protocol::Letter::Accepted {
        link,
        card: vec![1],
    };
    for _ in 0..MAX_LETTERS_FROM_ONE {
        leave(&database, to, Some(from), &card(Uuid::new_v4())).unwrap();
    }
    let refused = leave(&database, to, Some(from), &card(Uuid::new_v4()));
    assert!(
        matches!(refused, Err(ServerError::Refused(ErrorCode::OverQuota, _))),
        "{refused:?}"
    );
    leave(
        &database,
        to,
        Some(other),
        &be_block_protocol::Letter::KeyChange(vec![0]),
    )
    .unwrap();
    leave(
        &database,
        to,
        None,
        &be_block_protocol::Letter::Notice("a notice".into()),
    )
    .unwrap();
    let held = ServerStore::open(&harness.directory)
        .unwrap()
        .mailbox(to)
        .await
        .unwrap();
    let changes: Vec<u8> = held
        .iter()
        .filter(|message| message.from == Some(from))
        .filter_map(|message| match &message.letter {
            be_block_protocol::Letter::KeyChange(change) => Some(change[0]),
            _ => None,
        })
        .collect();
    assert_eq!(
        changes,
        (1..=MAX_LETTERS_FROM_ONE as u8).collect::<Vec<_>>(),
        "the oldest change went and the newest stayed"
    );
    assert_eq!(held.len() as i64, 2 * MAX_LETTERS_FROM_ONE + 2);
}
