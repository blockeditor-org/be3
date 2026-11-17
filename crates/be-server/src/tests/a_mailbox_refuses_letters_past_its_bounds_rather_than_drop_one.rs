use super::*;
use crate::store::mailbox::{MAX_LETTERS_FROM_ONE, leave};

/// Letting an old key change go would break the chain a device follows, so past the bound on
/// letters from one sender the write that would leave another is refused, and every letter
/// already held stays. Other senders, and the server's notices, are not held back by it.
#[tokio::test]
async fn a_mailbox_refuses_letters_past_its_bounds_rather_than_drop_one() {
    let harness = Harness::start().await;
    let to = harness.member("to@example.com").await.id();
    let from = harness.member("from@example.com").await.id();
    let other = harness.member("other@example.com").await.id();
    let database = metadata_database(&harness.directory);
    let change = be_block_protocol::Letter::KeyChange(vec![7]);
    for _ in 0..MAX_LETTERS_FROM_ONE {
        leave(&database, to, Some(from), &change).unwrap();
    }
    let refused = leave(&database, to, Some(from), &change);
    assert!(
        matches!(refused, Err(ServerError::Refused(ErrorCode::OverQuota, _))),
        "{refused:?}"
    );
    leave(&database, to, Some(other), &change).unwrap();
    leave(
        &database,
        to,
        None,
        &be_block_protocol::Letter::Notice("a notice".into()),
    )
    .unwrap();
    let store = ServerStore::open(&harness.directory).unwrap();
    let held = store.mailbox(to).await.unwrap();
    assert_eq!(
        held.iter().filter(|message| message.from == Some(from)).count() as i64,
        MAX_LETTERS_FROM_ONE
    );
    assert_eq!(held.len() as i64, MAX_LETTERS_FROM_ONE + 2);
}
