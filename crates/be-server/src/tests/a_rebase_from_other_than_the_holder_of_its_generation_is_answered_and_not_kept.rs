use be_block_protocol::JournalBase;

use super::*;

/// A holder whose lease lapsed or passed seals on until the next `Lease` tells it: the server
/// answers its rebase as done, and keeps the journal as it was.
#[tokio::test]
async fn a_rebase_from_other_than_the_holder_of_its_generation_is_answered_and_not_kept() {
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
    let base = JournalBase {
        commit: Some(first.0),
        sequence: 0,
        state: Vec::new(),
    };
    phone
        .submit(block, 7, 1, 0, b"an edit".to_vec())
        .await
        .unwrap();

    let answered = phone.rebase_journal(block, generation + 1, base).await;
    assert!(
        answered.is_ok(),
        "a rebase of a generation it does not own is answered: {answered:?}"
    );
    let journal = journal_of(&mut phone, block).await;
    assert_eq!(
        journal.map(|journal| journal.entries.len()),
        Some(1),
        "the journal is kept as it was"
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
