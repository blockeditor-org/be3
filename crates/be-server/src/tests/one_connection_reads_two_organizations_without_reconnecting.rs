use super::*;

/// No organization is open on a connection: each call names its own, so one connection reads
/// and writes in two organizations in turn, and hears the trees it follows in both.
#[tokio::test]
async fn one_connection_reads_two_organizations_without_reconnecting() {
    let harness = Harness::start().await;
    let mut client = harness.member("member@example.com").await;
    let personal = client.personal().await;
    let mine = client.create_block(BlockParent::Root).await;
    client.follow(mine).await;
    let team = client.organization("team").await;
    let ours = client.create_block(BlockParent::Root).await;
    client.follow(ours).await;

    let peer = client.peer.clone();
    let summaries = |organization: Uuid, block: Uuid| {
        peer.read_in::<Vec<Option<BlockSummary>>>(
            Some(organization),
            Query::Blocks {
                blocks: vec![block],
                contents: false,
                held: Vec::new(),
            },
        )
    };
    for (organization, block, other) in [(personal, mine, ours), (team, ours, mine)] {
        let read = summaries(organization, block).await.unwrap();
        assert!(
            matches!(&read, Ok(found) if found[0].as_ref().is_some_and(|found| found.id == block)),
            "{read:?}"
        );
        let read = summaries(organization, other).await.unwrap();
        assert!(
            matches!(&read, Ok(found) if found[0].is_none()),
            "a block of one organization was read in the other: {read:?}"
        );
    }

    for (organization, block) in [(personal, mine), (team, ours)] {
        let written = peer
            .write_in::<BlockSummary>(
                Some(organization),
                Op::Publish {
                    block,
                    expected: None,
                    commit: None,
                    references: be_block_protocol::ReferenceChange::between(&[], &Vec::new()),
                    metadata: b"changed".to_vec(),
                    journal: None,
                },
            )
            .await
            .unwrap();
        assert!(written.is_ok(), "{written:?}");
    }
    let mut heard = std::collections::BTreeSet::new();
    let events = client.heard.as_mut().expect("the client listens");
    while heard.len() < 2 {
        if let Event::TreeAdvanced {
            organization,
            entry,
            ..
        } = events.recv().await.expect("the events stay open")
        {
            heard.insert((organization, entry));
        }
    }
    assert_eq!(
        heard,
        [(personal, mine), (team, ours)].into_iter().collect(),
        "each tree advance names its organization"
    );

    harness.stop().await;
}
