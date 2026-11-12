use be_block_graph::Access;

use super::*;

/// One account attacks another's block with what it can learn of it: its id and the hashes of its
/// objects. Nothing it does tells it anything, reaches the owner's connections, or changes the
/// block.
#[tokio::test]
async fn another_account_cannot_reach_a_block_by_its_id_or_its_objects_by_their_hashes() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    owner.personal().await;
    let author = Author::new(account);
    let block = owner.create_block(BlockParent::Root).await;
    let (commit, chunks) = author.compose(b"the owner's words\n", 1_000, None);
    let published = owner
        .publish(&author, block, commit, chunks.clone(), None, 1_000)
        .await;
    assert!(
        matches!(
            published,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{published:?}"
    );
    let (unheld, loose) = author.compose(b"uploaded, never published\n", 2_000, Some(commit));
    owner
        .upload(&author.store, &author.objects_of(unheld, &loose))
        .await;
    owner.watch(block).await;
    let (_, session) = owner.join_session(block).await;
    let before = owner.read_block(block).await;

    // In an organization of its own, the owner's block is not there.
    let mut attacker = harness.client().await;
    attacker.register("attacker@example.com").await;
    let elsewhere = attacker.organization("elsewhere").await;
    let read = attacker
        .call(|peer| client::blocks::read_block(peer, block))
        .await;
    assert!(refused_as(&read, ErrorCode::BlockNotFound), "{read:?}");
    let contents = attacker
        .call(|peer| client::blocks::read_contents(peer, vec![block], Vec::new()))
        .await;
    assert!(
        refused_as(&contents, ErrorCode::BlockNotFound),
        "{contents:?}"
    );
    let joined = attacker.try_join(block).await;
    assert!(refused_as(&joined, ErrorCode::BlockNotFound), "{joined:?}");

    // A hash it never uploaded is missing to it, and is not its to hold, publish or read.
    let mut hashes = author.objects_of(commit, &chunks);
    hashes.extend(author.objects_of(unheld, &loose));
    let missing = attacker
        .call(|peer| client::objects::missing_objects(peer, hashes.clone()))
        .await;
    assert_eq!(missing.as_ref().ok(), Some(&hashes), "{missing:?}");
    // Asking to upload what the server already holds is not having it, even with every length
    // right; nor is asking for an object its uploader has not used yet.
    let known: Vec<Vec<u8>> = hashes
        .iter()
        .map(|hash| author.store.get(*hash).unwrap().unwrap())
        .collect();
    let reserved = attacker.reserve(&known).await;
    assert!(reserved.is_ok(), "{reserved:?}");
    let decoy = attacker.create_block(BlockParent::Root).await;
    let held = attacker
        .call(|peer| client::objects::hold_objects(peer, decoy, hashes.clone(), Hold::Lasting))
        .await;
    assert!(refused_as(&held, ErrorCode::ObjectNotFound), "{held:?}");
    let stolen = attacker
        .call(|peer| {
            client::blocks::publish(
                peer,
                decoy,
                commit,
                None,
                Vec::new(),
                objects_change(&[], chunks.clone()),
                1_000,
                false,
                listing(&[], Vec::new()),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(refused_as(&stolen, ErrorCode::ObjectNotFound), "{stolen:?}");
    for hash in &hashes {
        let read = attacker.download(*hash).await;
        assert!(
            matches!(read, Ok(None)),
            "the attacker read {hash}: {read:?}"
        );
    }
    let contents = attacker
        .call(|peer| client::blocks::read_contents(peer, vec![decoy], Vec::new()))
        .await;
    assert!(
        matches!(&contents, Ok(contents) if contents.objects.is_empty()),
        "{contents:?}"
    );

    // A block with the owner's id in the attacker's organization is a block of its own: its
    // watchers and session are not the owner's.
    let copy = attacker.read_block(decoy).await;
    let key = Uuid::new_v4();
    attacker
        .create_keyed(
            block,
            BlockParent::Root,
            key,
            KeySet {
                keys: vec![row(key, block, KeyKind::Data)],
                wrapped: Vec::new(),
            },
        )
        .await;
    attacker.watch(block).await;
    let (impostor, in_copy) = attacker.join_session(block).await;
    assert_eq!(
        in_copy.participants,
        vec![impostor],
        "the attacker joined the owner's session"
    );
    let forged = Author::new(copy.author.unwrap());
    let (fake, fake_chunks) = forged.compose(b"forged\n", 3_000, None);
    let response = attacker
        .publish(&forged, block, fake, fake_chunks, None, 3_000)
        .await;
    assert!(
        matches!(
            response,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{response:?}"
    );
    let relayed = attacker
        .relay(block, None, RelayKind::Edit, b"forged edit".to_vec()).await;
    assert!(matches!(relayed, Ok(())), "{relayed:?}");
    let deleted = attacker
        .call(|peer| {
            client::blocks::set_parent(peer, block, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(deleted, Ok(())), "{deleted:?}");
    let _ = elsewhere;

    // Inside the owner's organization, an account it shared the block with to view may not take
    // its lease, nor change it.
    let mut viewer = harness.client().await;
    let viewer_account = viewer.register("viewer@example.com").await;
    viewer.personal().await;
    let link = owner.link(None).await;
    let accepted = viewer.accept(link, &[]).await;
    assert!(matches!(accepted, Ok(())), "{accepted:?}");
    let granted = owner.grant(block, viewer_account, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    viewer.join_session(block).await;
    let claimed = viewer.sessions[&block]
        .claim_ownership(session.generation + 1)
        .await
        .expect("the connection stayed open");
    assert!(
        refused_as(&claimed, ErrorCode::PermissionDenied),
        "{claimed:?}"
    );
    let stolen = viewer
        .call(|peer| client::objects::hold_objects(peer, block, vec![unheld.hash()], Hold::Lasting))
        .await;
    assert!(
        refused_as(&stolen, ErrorCode::PermissionDenied),
        "{stolen:?}"
    );

    // The owner heard nothing of any of it, and its block is as it was.
    owner.round_trip().await;
    let heard: Vec<&Notice> = owner
        .notifications
        .iter()
        .filter(|notice| {
            matches!(
                notice,
                Notice::HeadChanged { .. } | Notice::BlockDeleted { .. } | Notice::Relayed { .. }
            )
        })
        .collect();
    assert!(heard.is_empty(), "the owner heard the attacker: {heard:?}");
    let after = owner.read_block(block).await;
    assert_eq!(
        (after.head, after.parent, after.metadata),
        (before.head, before.parent, before.metadata)
    );
    let state = owner
        .heartbeat(block, session.generation, None)
        .await
        .expect("the owner still beats");
    assert_eq!(state.owner, session.owner, "the lease changed hands");
    assert!(
        !state.participants.contains(&impostor),
        "the attacker is in the owner's session"
    );

    harness.stop().await;
}
