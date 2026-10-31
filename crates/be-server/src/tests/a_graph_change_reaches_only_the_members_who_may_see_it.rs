use super::*;
use be_graph::Access;

async fn join(
    harness: &Harness,
    owner: &mut TestClient,
    organization: Uuid,
    email: &str,
    invitation: u128,
) -> (TestClient, Uuid) {
    let mut member = harness.client().await;
    let account = member.register(email).await;
    let response = owner
        .call(|peer| {
            client::organizations::invite(
                peer,
                Uuid::from_u128(invitation),
                organization,
                email.into(),
                OrganizationRole::Editor,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let _ = member
        .call(|peer| {
            client::organizations::accept_invitation(
                peer,
                Uuid::from_u128(invitation),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    member.open(organization).await;
    (member, account)
}

async fn grant(owner: &mut TestClient, block: Uuid, account: Uuid, access: Access) {
    let response = owner
        .call(|peer| client::blocks::set_access(peer, block, account, access))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
}

async fn changes(client: &mut TestClient, count: usize) -> Vec<(Uuid, bool)> {
    let mut seen = Vec::new();
    for _ in 0..count {
        seen.push(match client.graph_change().await {
            Notice::BlockChanged { block } => (block.id, true),
            Notice::BlockRemoved { block, .. } => (block, false),
            other => panic!("{other:?}"),
        });
    }
    seen.sort();
    seen
}

#[tokio::test]
async fn a_graph_change_reaches_only_the_members_who_may_see_it() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("shared").await;
    let folder = owner.create_block(BlockParent::Root).await;
    let inside = owner.create_block(BlockParent::Block(folder)).await;
    let (mut viewer, account) =
        join(&harness, &mut owner, organization, "viewer@example.com", 1).await;
    let (mut outsider, _) = join(
        &harness,
        &mut owner,
        organization,
        "outsider@example.com",
        2,
    )
    .await;
    viewer.list(BlockParent::Root).await;
    outsider.list(BlockParent::Root).await;

    grant(&mut owner, folder, account, Access::View).await;
    assert_eq!(
        changes(&mut viewer, 1).await,
        [(folder, true)],
        "a block inside a folder is heard of only once the folder is listed"
    );
    let listed = viewer.list(BlockParent::Block(folder)).await;
    assert_eq!(
        listed.iter().map(|block| block.id).collect::<Vec<_>>(),
        [inside]
    );

    let response = owner
        .call(|peer| {
            client::blocks::set_metadata(peer, inside, b"sealed name".to_vec(), Vec::new())
        })
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");
    assert_eq!(changes(&mut viewer, 1).await, [(inside, true)]);

    grant(&mut owner, folder, account, Access::None).await;
    let mut expected = vec![(folder, false), (inside, false)];
    expected.sort();
    assert_eq!(changes(&mut viewer, 2).await, expected);

    let own = outsider.create_block(BlockParent::Root).await;
    assert_eq!(
        changes(&mut outsider, 1).await,
        [(own, true)],
        "the outsider heard of blocks it may not see"
    );

    harness.stop().await;
}
