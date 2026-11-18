use super::*;

/// One read of many logs' redactions names, for each, how many it has and the places of those
/// since the count asked; a read asking of more logs than `MAX_LOG_REDACTIONS_ASKED` is
/// refused, as other list reads are.
#[tokio::test]
async fn a_log_redactions_read_names_what_was_redacted_since_for_at_most_its_cap() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let chat = owner.create_block(BlockParent::Root).await;
    owner
        .append(
            chat,
            vec![log_entry(b"one"), log_entry(b"two"), log_entry(b"three")],
        )
        .await
        .unwrap();
    for seq in [2, 0] {
        let redacted = owner
            .call(|peer| peer.write::<()>(Op::Redact { block: chat, seq }))
            .await;
        assert!(redacted.is_ok(), "{redacted:?}");
    }
    let ask = |after: u64| be_block_protocol::LogRedactionsQuery { block: chat, after };
    let since_none = owner
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::LogRedactions>>(Query::LogRedactions(vec![ask(0)]))
        })
        .await
        .unwrap();
    assert_eq!(since_none.len(), 1);
    assert_eq!(since_none[0].redactions, 2);
    assert_eq!(since_none[0].redacted, [2, 0], "in the order they were redacted");
    let since_one = owner
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::LogRedactions>>(Query::LogRedactions(vec![ask(1)]))
        })
        .await
        .unwrap();
    assert_eq!(since_one[0].redacted, [0]);

    let at_cap = vec![ask(2); be_block_protocol::MAX_LOG_REDACTIONS_ASKED];
    let read = owner
        .call(|peer| peer.read::<Vec<be_block_protocol::LogRedactions>>(Query::LogRedactions(at_cap)))
        .await;
    assert!(read.is_ok(), "{read:?}");
    let past_cap = vec![ask(2); be_block_protocol::MAX_LOG_REDACTIONS_ASKED + 1];
    let refused = owner
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::LogRedactions>>(Query::LogRedactions(past_cap))
        })
        .await;
    assert!(
        matches!(&refused, Err(error) if error.code == ErrorCode::OverQuota),
        "{refused:?}"
    );
    harness.stop().await;
}
