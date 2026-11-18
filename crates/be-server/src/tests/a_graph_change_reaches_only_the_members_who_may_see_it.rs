use super::*;
use be_block_graph::Access;

pub(super) async fn join(
    harness: &Harness,
    owner: &mut TestClient,
    organization: Uuid,
    email: &str,
    invitation: u128,
) -> (TestClient, Uuid) {
    let mut member = harness.member(email).await;
    let account = member.id();
    let response = owner
        .invite_with(
            Uuid::from_u128(invitation),
            organization,
            Vec::new(),
            Vec::new(),
        )
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let _ = member
        .accept(Uuid::from_u128(invitation), &Vec::new())
        .await;
    member.open(organization).await;
    (member, account)
}

async fn grant(owner: &mut TestClient, block: Uuid, account: Uuid, access: Access) {
    let response = owner.grant(block, account, access).await;
    assert!(matches!(response, Ok(())), "{response:?}");
}

/// The next `tree-advanced` or `roots` told, skipping nothing.
async fn told(client: &mut TestClient) -> Notice {
    match client.graph_change().await {
        Notice::TreeAdvanced { entry, .. } => Notice::TreeAdvanced { entry, sequence: 0 },
        other => other,
    }
}

#[tokio::test]
async fn a_graph_change_reaches_only_the_members_who_may_see_it() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
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
        told(&mut viewer).await,
        Notice::Roots,
        "a grant changes what the grantee finds at the top"
    );
    assert_eq!(viewer.follow(inside).await, folder);
    let listed = viewer.list(BlockParent::Block(folder)).await;
    assert_eq!(
        listed.iter().map(|block| block.id).collect::<Vec<_>>(),
        [inside]
    );

    let response = owner
        .call(|peer| {
            peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::Publish {
                block: inside,
                expected: None,
                commit: None,
                references: be_block_protocol::ReferenceChange::between(&[], &Vec::new()),
                metadata: b"sealed name".to_vec(),
                journal: None,
            })
        })
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");
    assert_eq!(
        told(&mut viewer).await,
        Notice::TreeAdvanced {
            entry: folder,
            sequence: 0
        }
    );
    let (changed, _) = viewer.read_changes(folder).await;
    assert!(changed.iter().any(|block| block.id == inside));

    grant(&mut owner, folder, account, Access::None).await;
    assert_eq!(
        told(&mut viewer).await,
        Notice::TreeAdvanced {
            entry: folder,
            sequence: 0
        },
        "a follower that lost view is told once, to read the tree and be refused"
    );
    assert_eq!(told(&mut viewer).await, Notice::Roots);
    let response = viewer
        .call(|peer| {
            peer.ask::<be_block_protocol::ChangedPage>(
                be_block_protocol::Query::Tree {
                    entry: folder,
                    after: be_block_protocol::ChangeCursor {
                        changed: 0,
                        block: Uuid::max(),
                    },
                    limit: 10,
                },
                false,
            )
        })
        .await;
    assert!(
        response.is_err(),
        "a tree is read only by who may view it: {response:?}"
    );
    owner.create_block(BlockParent::Block(folder)).await;
    owner.round_trip().await;
    viewer.round_trip().await;
    assert!(
        !viewer
            .notifications
            .iter()
            .any(|notice| matches!(notice, Notice::TreeAdvanced { .. })),
        "and it follows the tree no longer"
    );

    let own = outsider.create_block(BlockParent::Root).await;
    assert_eq!(
        told(&mut outsider).await,
        Notice::Roots,
        "the outsider heard of blocks it may not see"
    );
    assert_eq!(outsider.follow(own).await, own);

    harness.stop().await;
}
