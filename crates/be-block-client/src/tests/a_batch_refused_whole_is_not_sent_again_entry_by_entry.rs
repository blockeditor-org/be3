use super::*;

/// A batch of entries the server refuses for the whole call (its author demoted meanwhile, as
/// a quota or the entries a minute would too) is kept refused after one write: each entry alone
/// would be refused the same, and would spend what refused it again.
#[tokio::test]
async fn a_batch_refused_whole_is_not_sent_again_entry_by_entry() {
    let harness = Harness::start().await;
    let revoked = revoked(&harness).await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&revoked.carol, &refs, &store).await;
    assert_eq!(text(&online, revoked.note).await, "before");
    drop(online);
    let offline = harness.offline(&revoked.carol, &refs, &store);
    let mut kept = Vec::new();
    for text in ["one", "two", "three"] {
        match offline
            .append(
                revoked.note,
                "test.app",
                text.as_bytes(),
                be_block_content::Along::default(),
            )
            .await
            .unwrap()
        {
            Appended::Kept { id } => kept.push(id),
            appended => panic!("an entry made offline was not kept: {appended:?}"),
        }
    }
    revoked
        .admin
        .grant(revoked.team, revoked.carol.account(), Access::View)
        .await
        .unwrap();
    revoked.admin.tend().await.unwrap();

    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = be_block_fault_relay::Relay::start(server);
    let (connection, signed_in) = Connection::connect(
        &relay.url(),
        &Clock::real(),
        &Credentials::Token(revoked.carol.token()),
    )
    .await
    .expect("the server signs the device in");
    offline.attach(connection, signed_in.token).await.unwrap();
    let writes = relay.count(|method, _| method == be_block_protocol::__args::SERVER_WRITE);
    let drained = offline.drain(|_| false).await.unwrap();
    let refused: Vec<Uuid> = drained
        .iter()
        .filter_map(|outcome| match outcome {
            Drained::NotAppended { block, id, .. } if *block == revoked.note => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(refused, kept, "{drained:?}");
    assert_eq!(
        *writes.borrow(),
        1,
        "the refused batch went again entry by entry"
    );
    assert_eq!(offline.pending().operations, 0);
    assert_eq!(
        offline
            .refused_entries(revoked.note)
            .into_iter()
            .map(|refused| refused.id)
            .collect::<Vec<_>>(),
        kept
    );
    harness.stop().await;
}
