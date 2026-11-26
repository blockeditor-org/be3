use be_block_protocol::JournalBase;

use super::*;

/// A seal holds only what the order numbered: a trim onto a base past its last entry is not
/// kept, though its publish goes through, and the order goes on numbering after what it holds.
#[tokio::test]
async fn a_trim_past_the_orders_last_entry_is_not_kept() {
    let harness = Harness::start().await;
    let mut phone = harness.member("phone@example.com").await;
    let account = phone.id();
    phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let first = author.compose(b"saved\n", 1_000, None);
    let _ = phone
        .publish(&author, block, first.0, first.1.clone(), None, 1_000)
        .await;
    phone.join_session(block).await;
    phone
        .submit(block, 7, 1, 0, b"an edit".to_vec())
        .await
        .unwrap();

    let past = JournalBase {
        commit: Some(first.0),
        sequence: 5,
        state: Vec::new(),
        parts: Vec::new(),
    };
    let answered = phone.trim(&author, block, first, of_build(past)).await;
    assert!(answered.is_ok(), "{answered:?}");

    phone
        .submit(block, 7, 2, 1, b"another".to_vec())
        .await
        .unwrap();
    let journal = phone
        .journal_of(block)
        .await
        .expect("the block has a journal");
    assert_eq!(
        journal
            .entries
            .iter()
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>(),
        [1, 2],
        "the order went on from its last entry"
    );
}
