use be_block_protocol::JournalBase;

use super::*;

/// A trim with no new commit is the holder's alone, by the server's derivation at the order's
/// tip: another participant's is refused and the journal kept, and once that participant
/// claims the lease the old holder's is refused in turn.
#[tokio::test]
async fn a_trim_from_other_than_the_holder_is_refused_and_not_kept() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    let account = phone.register("phone@example.com").await;
    let organization = phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let first = author.compose(b"saved\n", 1_000, None);
    let _ = phone
        .publish(&author, block, first.0, first.1.clone(), None, 1_000)
        .await;
    phone.join_session(block).await;
    let base = JournalBase {
        commit: Some(first.0),
        sequence: 1,
        state: Vec::new(),
    };
    phone
        .submit(block, 7, 1, 0, b"an edit".to_vec())
        .await
        .unwrap();
    let token = phone.token.clone();
    let mut laptop = harness.second_connection(&token, organization).await;
    laptop.join_session(block).await;

    let answered = laptop
        .trim(&author, block, first.clone(), base.clone())
        .await;
    assert!(answered.is_err(), "a participant that does not hold the lease trims: {answered:?}");
    let entries = |journal: Option<be_block_protocol::Journal>| {
        journal.map(|journal| journal.entries.len())
    };
    assert_eq!(entries(laptop.journal_of(block).await), Some(1));

    laptop.claim(block).await.unwrap();
    let answered = phone
        .trim(&author, block, first.clone(), base.clone())
        .await;
    assert!(answered.is_err(), "the old holder trims after a claim: {answered:?}");
    assert_eq!(entries(laptop.journal_of(block).await), Some(1));

    let answered = laptop.trim(&author, block, first, base).await;
    assert!(answered.is_ok(), "the claimant trims: {answered:?}");
    assert_eq!(
        entries(laptop.journal_of(block).await).unwrap_or(0),
        0,
        "the trim took the entry"
    );
}
