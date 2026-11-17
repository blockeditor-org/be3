use be_block_protocol::JournalBase;
use be_block_session::Membership;

use super::*;

/// A build's entries the holder of another build cannot read are dropped when it starts the
/// journal over, and every reader is told edits were lost until the journal moves to another
/// commit.
#[tokio::test]
async fn a_trim_by_another_build_marks_the_journal_lost_until_the_next_commit() {
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
    let old_build = Hash::from_bytes([1; 32]);
    let new_build = TYPE_BUILD;
    let base = |commit, sequence| JournalBase {
        commit: Some(commit),
        sequence,
        state: Vec::new(),
    };

    phone.try_join_as(block, old_build).await.unwrap();
    phone
        .submit(block, 7, 1, 0, b"an edit".to_vec())
        .await
        .unwrap();
    let journal = journal_of(&mut phone, block).await;
    assert!(journal.is_some_and(|journal| !journal.lost && journal.entries.len() == 1));
    let token = phone.token.clone();
    drop(phone);

    let mut laptop = harness.second_connection(&token, organization).await;
    let (_, state) = laptop.try_join_as(block, new_build).await.unwrap();
    assert_eq!(state.tag(), Some(new_build));
    let trimmed = laptop
        .trim(&author, block, first.clone(), of_build(base(first.0, 0)))
        .await;
    assert!(trimmed.is_ok(), "{trimmed:?}");
    let journal = journal_of(&mut laptop, block).await;
    assert!(
        journal.is_some_and(|journal| journal.lost && journal.entries.is_empty()),
        "another build's entries were dropped and the block says so"
    );

    let second = author.compose(b"saved again\n", 2_000, Some(first.0));
    let sealed = laptop
        .seal(
            &author,
            block,
            second.0,
            second.1,
            Some(first.0),
            2_000,
            Some(of_build(base(second.0, 0))),
        )
        .await;
    assert!(
        matches!(sealed, Ok(be_block_protocol::PublishOutcome::Published(_))),
        "{sealed:?}"
    );
    let journal = journal_of(&mut laptop, block).await;
    assert!(
        journal.is_none_or(|journal| !journal.lost),
        "the next commit clears the mark"
    );
}

/// What a reader of `block` is told of its journal.
async fn journal_of(client: &mut TestClient, block: Uuid) -> Option<be_block_protocol::Journal> {
    let response = client
        .call(|peer| peer.read::<be_block_protocol::Contents>(be_block_protocol::Query::Blocks { blocks: vec![block], contents: true, held: Vec::new() }))
        .await;
    let Ok(contents) = response else {
        panic!("reading failed: {response:?}");
    };
    contents.journals.into_iter().next().flatten()
}
