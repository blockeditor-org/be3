use be_block_protocol::JournalBase;

use super::*;

/// Once a new build started the journal over, a seal the old build drains afterwards publishes
/// and trims nothing: it cannot have applied the new build's entries, which stay.
#[tokio::test]
async fn a_seal_by_the_old_build_after_a_new_build_started_over_trims_nothing() {
    let harness = Harness::start().await;
    let mut phone = harness.member("phone@example.com").await;
    let account = phone.id();
    let organization = phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);
    let old_build = Hash::from_bytes([1; 32]);
    let base = |commit, sequence| JournalBase {
        commit: Some(commit),
        sequence,
        state: Vec::new(),
        parts: Vec::new(),
    };
    let first = author.compose(b"saved\n", 1_000, None);
    let _ = phone
        .publish(&author, block, first.0, first.1.clone(), None, 1_000)
        .await;
    phone.try_join_as(block, old_build).await.unwrap();
    phone
        .submit(block, 7, 1, 0, b"old build".to_vec())
        .await
        .unwrap();
    let token = phone.token.clone();
    phone.leave(block).await;

    let mut laptop = harness.second_connection(&token, organization).await;
    laptop.join_session(block).await;
    let started = laptop
        .trim(&author, block, first.clone(), of_build(base(first.0, 1)))
        .await;
    assert!(started.is_ok(), "{started:?}");
    laptop
        .submit(block, 8, 1, 1, b"new build".to_vec())
        .await
        .unwrap();
    let kept = |journal: Option<be_block_protocol::Journal>| {
        journal.map(|journal| {
            journal
                .entries
                .iter()
                .map(|entry| entry.payload.clone())
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(
        kept(laptop.journal_of(block).await),
        Some(vec![b"new build".to_vec()])
    );

    let stale = author.compose(b"sealed by the old build\n", 2_000, Some(first.0));
    let answered = phone
        .seal(
            &author,
            block,
            stale.0,
            stale.1,
            Some(first.0),
            2_000,
            Some(in_run(base(stale.0, 1), old_build, 0)),
        )
        .await;
    assert!(
        matches!(
            answered,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{answered:?}"
    );
    assert_eq!(
        kept(laptop.journal_of(block).await),
        Some(vec![b"new build".to_vec()]),
        "the old build's seal wiped the new build's entries"
    );
}
