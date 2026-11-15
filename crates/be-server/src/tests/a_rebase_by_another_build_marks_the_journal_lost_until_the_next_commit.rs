use be_block_protocol::JournalBase;

use super::*;

/// A build's entries the holder of another build cannot read are dropped when it starts the
/// journal over, and every reader is told edits were lost until the journal moves to another
/// commit.
#[tokio::test]
async fn a_rebase_by_another_build_marks_the_journal_lost_until_the_next_commit() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    let account = phone.register("phone@example.com").await;
    let organization = phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let first = author.compose(b"saved\n", 1_000, None);
    let _ = phone
        .publish(&author, block, first.0, first.1, None, 1_000)
        .await;
    let old_build = Hash::from_bytes([1; 32]);
    let new_build = Hash::from_bytes([2; 32]);
    let base = |commit| JournalBase {
        commit: Some(commit),
        sequence: 0,
        state: Vec::new(),
    };

    phone.try_join_as(block, old_build).await.unwrap();
    phone.submit(block, 7, 1, 0, b"an edit".to_vec()).await.unwrap();
    let journal = journal_of(&mut phone, block).await;
    assert!(journal.is_some_and(|journal| !journal.lost && journal.entries.len() == 1));
    let token = phone.token.clone();
    drop(phone);

    let mut laptop = harness.second_connection(&token, organization).await;
    let (_, state) = laptop.try_join_as(block, new_build).await.unwrap();
    assert_eq!(state.tag, Some(new_build));
    laptop
        .rebase_journal(block, state.generation, base(first.0))
        .await
        .unwrap();
    let journal = journal_of(&mut laptop, block).await;
    assert!(
        journal.is_some_and(|journal| journal.lost && journal.entries.is_empty()),
        "another build's entries were dropped and the block says so"
    );

    let second = author.compose(b"saved again\n", 2_000, Some(first.0));
    let _ = laptop
        .publish(&author, block, second.0, second.1, Some(first.0), 2_000)
        .await;
    laptop
        .rebase_journal(block, state.generation, base(second.0))
        .await
        .unwrap();
    let journal = journal_of(&mut laptop, block).await;
    assert!(
        journal.is_none_or(|journal| !journal.lost),
        "the next commit clears the mark"
    );
}

/// What a reader of `block` is told of its journal.
async fn journal_of(client: &mut TestClient, block: Uuid) -> Option<be_block_protocol::Journal> {
    let response = client
        .call(|peer| client::blocks::read_contents(peer, vec![block], Vec::new()))
        .await;
    let Ok(contents) = response else {
        panic!("reading failed: {response:?}");
    };
    contents.journals.into_iter().next().flatten()
}
