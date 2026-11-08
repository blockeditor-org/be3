use be_block_protocol::{JournalBase, JournalEntry};

use super::*;

#[tokio::test]
async fn a_rebase_by_another_build_marks_the_journal_lost_until_the_next_commit() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    let account = phone.register("phone@example.com").await;
    phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let first = author.compose(b"saved\n", 1_000, None);
    let _ = phone
        .publish(&author, block, first.0, first.1, None, 1_000)
        .await;
    let (_, state) = phone.join_session(block).await;
    let generation = state.generation;
    let old_build = Hash::from_bytes([1; 32]);
    let new_build = Hash::from_bytes([2; 32]);
    let base = |commit| JournalBase {
        commit: Some(commit),
        sequence: 0,
        state: Vec::new(),
    };

    phone.sessions[&block]
        .rebase_journal(generation, old_build, base(first.0))
        .await
        .unwrap()
        .unwrap();
    phone.sessions[&block]
        .append(
            generation,
            old_build,
            vec![JournalEntry {
                generation,
                sequence: 1,
                payload: b"an edit".to_vec(),
            }],
            false,
        )
        .await
        .unwrap()
        .unwrap();
    let journal = journal_of(&mut phone, block).await;
    assert!(journal.is_some_and(|journal| !journal.lost));

    phone.sessions[&block]
        .rebase_journal(generation, new_build, base(first.0))
        .await
        .unwrap()
        .unwrap();
    let journal = journal_of(&mut phone, block).await;
    assert!(
        journal.is_some_and(|journal| journal.lost && journal.entries.is_empty()),
        "another build's entries were dropped and the block says so"
    );

    let second = author.compose(b"saved again\n", 2_000, Some(first.0));
    let _ = phone
        .publish(&author, block, second.0, second.1, Some(first.0), 2_000)
        .await;
    phone.sessions[&block]
        .rebase_journal(generation, new_build, base(second.0))
        .await
        .unwrap()
        .unwrap();
    let journal = journal_of(&mut phone, block).await;
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
