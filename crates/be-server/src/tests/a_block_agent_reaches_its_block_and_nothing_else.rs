use super::every_call_states_who_may_make_it::Answered;
use super::*;
use be_block_graph::Access;

/// Calls that reach past one block, which a block's agent may not make whatever they name.
const REACHING_PAST: &[&str] = &[
    "begin-browser-login",
    "poll-browser-login",
    "create-local-profile",
    "storage-usage",
    "storage-changed",
    "pair",
    "paired",
    "create-organization",
    "invite",
    "get-invitation",
    "accept-invitation",
    "list-members",
    "set-role",
    "remove-member",
    "create-agent",
    "list-agents",
    "rotate-agent-token",
    "remove-agent",
    "create-contact-link",
    "get-contact-link",
    "accept-contact-link",
    "list-contact-links",
    "delete-contact-link",
    "put-contact",
    "remove-contact",
    "set-parent",
    "collect-detached",
    "set-access",
    "list-access",
    "set-recovery-key",
    "create-escrow",
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
    "missing-objects",
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
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let terminal = owner.create_block(BlockParent::Root).await;
    let elsewhere = owner.create_block(BlockParent::Root).await;
    let under = owner.create_block(BlockParent::Block(elsewhere)).await;
    let (account, token) = owner.block_agent(organization, terminal).await.unwrap();

    let response = owner
        .call(|peer| client::members::list_agents(peer, organization))
        .await;
    assert!(
        matches!(&response, Ok(agents) if agents.is_empty()),
        "a block's agent is listed with the creator's own: {response:?}"
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
        let response = agent
            .call(|peer| client::blocks::read_block(peer, block))
            .await;
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
        .call(|peer| client::blocks::delete_block(peer, terminal))
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
    assert!(after.head.is_none(), "a block's agent published elsewhere");

    harness.stop().await;
}
