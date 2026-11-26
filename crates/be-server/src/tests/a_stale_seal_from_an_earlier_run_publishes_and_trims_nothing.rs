use be_block_protocol::JournalBase;

use super::*;

/// Sequence numbers are reused once the order starts over, so a seal of an earlier run, which
/// a device drains after the last one out restarted the order and a new run was numbered,
/// names other entries: its publish goes through on its compare-and-swap, and the journal stays
/// as it was, whether the stale seal reaches past the new run's last number or not.
#[tokio::test]
async fn a_stale_seal_from_an_earlier_run_publishes_and_trims_nothing() {
    let harness = Harness::start().await;
    let mut phone = harness.member("phone@example.com").await;
    let account = phone.id();
    phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    let author = Author::new(account);
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
    phone.join_session(block).await;
    for counter in 1..=2 {
        phone
            .submit(block, 7, counter, counter - 1, b"run zero".to_vec())
            .await
            .unwrap();
    }
    let sealed = author.compose(b"sealed run zero\n", 2_000, Some(first.0));
    let answered = phone
        .seal(
            &author,
            block,
            sealed.0,
            sealed.1.clone(),
            Some(first.0),
            2_000,
            Some(of_build(base(sealed.0, 2))),
        )
        .await;
    assert!(answered.is_ok(), "{answered:?}");

    // The last one out restarts the order on the head, and a new run is numbered from 1.
    let restarted = phone
        .trim(
            &author,
            block,
            sealed.clone(),
            in_run(base(sealed.0, 0), TYPE_BUILD, 1),
        )
        .await;
    assert!(restarted.is_ok(), "{restarted:?}");
    for counter in 3..=5 {
        phone
            .submit(block, 7, counter, counter - 3, b"run one".to_vec())
            .await
            .unwrap();
    }
    let numbered = |journal: Option<be_block_protocol::Journal>| {
        let journal = journal.expect("the new run's entries are kept");
        (
            journal.base.sequence,
            journal
                .entries
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(numbered(phone.journal_of(block).await), (0, vec![1, 2, 3]));

    // Stale seals of run zero on the same head, one within the new run's numbers and one past
    // them: each publishes, and neither trims.
    for (text, through) in [(&b"stale within\n"[..], 2), (&b"stale past\n"[..], 7)] {
        let head = phone.read_block(block).await.head.unwrap();
        let stale = author.compose(text, 3_000, Some(head));
        let answered = phone
            .seal(
                &author,
                block,
                stale.0,
                stale.1,
                Some(head),
                3_000,
                Some(of_build(base(stale.0, through))),
            )
            .await;
        assert!(
            matches!(answered, Ok(be_block_protocol::PublishOutcome::Published(at)) if at == stale.0),
            "a stale seal still publishes: {answered:?}"
        );
        assert_eq!(
            numbered(phone.journal_of(block).await),
            (0, vec![1, 2, 3]),
            "a stale seal trimmed the new run"
        );
    }
}
