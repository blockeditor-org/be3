use super::*;

/// An agent of `owner`'s bound to `block`, which grants it Edit there: its keys and token.
async fn bound_agent(
    owner: &Peer<MemoryStore>,
    block: Uuid,
) -> (Uuid, be_block_keys::RecoverySecret, String) {
    let secret = RecoveryPhrase::generate().secret();
    let account = Uuid::from_bytes(be_block_keys::agent_account(
        owner.account().into_bytes(),
        &secret.identity(),
    ));
    let organization = owner.organization();
    let bound = be_block_protocol::BoundAgent {
        block,
        signed: owner
            .grant_record(block, account, Access::Edit, secret.identity())
            .await
            .unwrap(),
    };
    let card = crate::contacts::card(
        owner.account(),
        owner.keys().identity(),
        "Owner",
        "owner@example.com",
    );
    let created = crate::ask(be_block_protocol::write::<be_block_protocol::CreatedAgent, _>(
        &owner.connection(),
        Some(organization),
        be_block_protocol::Op::CreateAgent {
            account,
            name: "Helper".into(),
            recovery: secret.public().0,
            signing: secret.signing().0,
            contact: crate::contacts::OwnContact::new(card, 0).seal(&secret),
            record: Vec::new(),
            escrow: crate::contacts::seal_pin(&secret, organization, &owner.keys().pins()),
            bound_to: Some(bound),
            member: owner.agent_record(account, secret.identity()).await.unwrap(),
        },
    ))
    .await
    .unwrap();
    (account, secret, created.token)
}

/// A block's bound agent, made with Edit and then lowered to View, keeps the key it reads with.
/// The lowering outdates the block's epoch, so the owner's next save rotates it, and the
/// rotation seals the new epoch to the agent at once, with no `tend` between: the agent,
/// connected since before, seals what it sends in the block's session under it (it may not
/// rotate the epoch itself) and opens the block read-only.
#[tokio::test]
async fn a_bound_agent_lowered_to_view_still_opens_its_block() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let block = owner
        .create_block(
            Uuid::now_v7(),
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::named("Note"),
            false,
        )
        .await
        .unwrap()
        .id;
    let (viewing, secret, token) = bound_agent(&owner, block).await;
    owner
        .share(block, viewing, Access::View, &secret.identity())
        .await
        .unwrap();
    let agent = Peer::connect(
        PeerConfig::new(&harness.url, secret, Credentials::Token(token))
            .organization(Some(owner.organization()))
            .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    assert_eq!(agent.summary(block).await.unwrap().access, Access::View);

    // Another agent's grant has the owner read the log again, past the lowering.
    let epoch = owner.keys().scope_key(block);
    let (editing, editing_secret, _) = bound_agent(&owner, block).await;
    owner
        .seal_share(block, editing, &editing_secret.public())
        .await
        .unwrap();
    owner
        .save(block, &TextBlock::of("kept"), None)
        .await
        .unwrap();
    assert_ne!(
        owner.keys().scope_key(block),
        epoch,
        "the save rotated the scope the lowering outdated"
    );

    let relayed = agent.relay(block, None, b"shown").await;
    assert!(
        relayed.is_ok(),
        "the agent could not seal under the new epoch: {:?}",
        relayed.err()
    );
    assert_eq!(text(&agent, block).await, "kept");

    harness.stop().await;
}
