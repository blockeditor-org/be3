use super::every_call_states_who_may_make_it::Answered;
use super::*;
use be_block_graph::Access;

/// Calls that reach past one block, which a block's agent may not make whatever they name.
const REACHING_PAST: &[&str] = &[
    "begin-browser-login",
    "poll-browser-login",
    "create-local-profile",
    "storage-changed",
    "pair",
    "paired",
    "create-organization",
    "list-members",
    "set-role",
    "remove-member",
    "create-agent",
    "rotate-agent-token",
    "create-link",
    "create-link (invitation)",
    "get-link",
    "accept-link",
    "list-links",
    "delete-link",
    "put-contact",
    "remove-contact",
    "set-parent",
    "collect-detached",
    "set-access",
    "list-access",
    "set-recovery-key",
    "get-signing-keys",
    "create-escrow",
    "push-key",
    "offers-fcm",
    "register-endpoint",
    "unregister-endpoint",
    "notify-modes",
    "notify-modes-changed",
    "set-notify-mode",
];

/// Calls that name no block, which a terminal host makes on connecting; each answers only
/// about the agent's own block.
const CONNECTING: &[&str] = &[
    "ping",
    "open-organization",
    "list-organizations",
    "list-contacts",
    "contacts-changed",
    "upload-urls",
    "download-urls",
    "list-blocks",
    "unlist",
    "changes",
    "get-keys",
    "put-keys",
    "get-recovery-key",
    "reseal-keys",
    "get-key-work",
    "key-work-changed",
    "pin-escrow",
    "logout",
];

#[tokio::test]
async fn a_block_agent_reaches_its_block_and_nothing_else() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let me = owner.register("owner@example.com").await;
    let organization = owner.organization("team").await;
    let terminal = owner.create_block(BlockParent::Root).await;
    let elsewhere = owner.create_block(BlockParent::Root).await;
    let under = owner.create_block(BlockParent::Block(elsewhere)).await;
    let (account, token) = owner.block_agent(organization, terminal).await.unwrap();

    let response = owner
        .call(|peer| client::members::list_members(peer))
        .await;
    assert!(
        matches!(&response, Ok(members) if members.iter().any(|member| member.account == account
            && member.created_by == Some(me)
            && member.bound_to == Some(terminal))),
        "a block's agent is listed as its block's: {response:?}"
    );
    let response = owner
        .call(|peer| client::blocks::list_access(peer, terminal))
        .await;
    let Ok(access) = response else {
        panic!("listing the block's access failed: {response:?}");
    };
    assert!(
        access
            .entries
            .iter()
            .any(|entry| entry.account == account && entry.granted == Some(Access::Edit)),
        "the block's access list does not show its agent with Edit"
    );

    let mut agent = harness.second_connection(&token, organization).await;
    assert_eq!(agent.read_block(terminal).await.access, Access::Edit);
    let inside = agent.create_block(BlockParent::Block(terminal)).await;
    assert_eq!(
        agent.read_block(inside).await.access,
        Access::Manage,
        "an agent manages what it makes inside its block, as any author does"
    );
    for block in [elsewhere, under] {
        let response = agent.try_read_block(block).await;
        assert!(
            refused_as(&response, ErrorCode::BlockNotFound),
            "a block's agent read another block: {response:?}"
        );
    }
    let response = agent
        .call(|peer| {
            client::blocks::create_block(
                peer,
                Uuid::new_v4(),
                CONTENT,
                BlockParent::Root,
                Vec::new(),
                Vec::new(),
                KEY,
                KeySet::default(),
                false,
            )
        })
        .await;
    assert!(
        refused_as(&response, ErrorCode::PermissionDenied),
        "a block's agent made a block at the root: {response:?}"
    );
    for (name, call) in [
        (
            "moving",
            client::blocks::set_parent(
                &agent.peer,
                terminal,
                BlockParent::Block(inside),
                KeySet::default(),
            ),
        ),
        (
            "deleting",
            client::blocks::set_parent(
                &agent.peer,
                terminal,
                BlockParent::Detached,
                KeySet::default(),
            ),
        ),
    ] {
        let response = call.await.expect("the connection stayed open");
        assert!(
            refused_as(&response, ErrorCode::PermissionDenied),
            "{name} its own block answered {response:?}"
        );
    }
    let response = agent
        .call(|peer| {
            client::blocks::set_parent(peer, terminal, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(
        refused_as(&response, ErrorCode::PermissionDenied),
        "a block's agent deleted its own block: {response:?}"
    );
    let listed = agent.list(BlockParent::Root).await;
    assert_eq!(
        listed.iter().map(|block| block.id).collect::<Vec<_>>(),
        [terminal],
        "the root lists more than the agent's block"
    );
    let keys = agent.keys(Some(vec![elsewhere, under])).await;
    assert!(
        keys.keys.is_empty() && keys.wrapped.is_empty(),
        "a block's agent was served another block's keys"
    );
    assert!(
        agent
            .keys(None)
            .await
            .keys
            .iter()
            .all(|key| [terminal, inside].contains(&key.block)),
        "a block's agent was served keys of blocks outside its own"
    );

    // What the calls a terminal host makes on connecting answer of the rest.
    let author = Author::new(me);
    let (head, chunks) = author.compose(b"somewhere else", 1_000, None);
    let published = owner
        .publish(&author, elsewhere, head, chunks, None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    agent.round_trip().await;
    assert!(
        agent.notifications.iter().all(|notice| match notice {
            Notice::BlockChanged { block, .. } => [terminal, inside].contains(&block.id),
            Notice::BlockRemoved { block, .. } => [terminal, inside].contains(block),
            _ => true,
        }),
        "a block's agent heard of another block"
    );
    assert!(
        matches!(agent.download(head.hash()).await, Ok(None)),
        "a block's agent was given another block's object"
    );
    let response = agent
        .missing(&[author.store.get(head.hash()).unwrap().unwrap()])
        .await;
    assert!(
        matches!(&response, Ok(missing) if *missing == [head.hash()]),
        "another block's object was not missing to its agent: {response:?}"
    );
    let own: Vec<Uuid> = agent
        .keys(None)
        .await
        .keys
        .iter()
        .map(|key| key.id)
        .collect();
    let response = agent.key_work().await;
    let Ok(work) = response else {
        panic!("reading the key work failed: {response:?}");
    };
    assert!(
        work.rotations
            .iter()
            .all(|block| [terminal, inside].contains(block))
            && work
                .unsealed
                .iter()
                .all(|unsealed| own.contains(&unsealed.key))
            && work.unescrowed.iter().all(|key| own.contains(key)),
        "a block's agent was given key work outside its block: {work:?}"
    );
    let response = agent
        .call(|peer| client::keys::get_recovery_key(peer))
        .await;
    let Ok(recovery) = response else {
        panic!("reading the recovery key failed: {response:?}");
    };
    assert!(
        recovery
            .sealed
            .iter()
            .all(|sealed| own.contains(&sealed.key)),
        "a block's agent was sealed another block's key"
    );
    assert!(
        agent
            .contacts()
            .await
            .iter()
            .all(|contact| contact.account == me),
        "a block's agent has contacts beyond its creator"
    );

    let answers = agent.everything(elsewhere, organization).await;
    for (name, answer) in &answers {
        if REACHING_PAST.contains(name) {
            assert!(
                matches!(
                    answer,
                    Answered::Denied | Answered::Refused(ErrorCode::PermissionDenied)
                ),
                "{name} answered a block's agent: {answer:?}"
            );
        } else if !CONNECTING.contains(name) && *name != "authenticate" {
            assert!(
                matches!(
                    answer,
                    Answered::Denied
                        | Answered::Refused(ErrorCode::PermissionDenied | ErrorCode::BlockNotFound)
                ),
                "{name} on another block answered its agent: {answer:?}"
            );
        }
    }
    let after = owner.read_block(elsewhere).await;
    assert_eq!(after.parent, Some(BlockParent::Root));
    assert_eq!(
        after.head,
        Some(head),
        "a block's agent published elsewhere"
    );

    harness.stop().await;
}
