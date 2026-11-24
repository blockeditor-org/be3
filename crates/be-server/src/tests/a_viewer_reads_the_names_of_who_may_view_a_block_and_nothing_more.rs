use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn a_viewer_reads_the_names_of_who_may_view_a_block_and_nothing_more() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    owner.personal().await;
    let mut viewer = harness.member("viewer@example.com").await;
    let viewer_id = viewer.id();
    viewer.personal().await;
    let mut knower = harness.member("knower@example.com").await;
    let knower_id = knower.id();
    knower.personal().await;
    let mut outsider = harness.member("outsider@example.com").await;
    outsider.personal().await;
    for invited in [&mut viewer, &mut knower] {
        let link = owner.link().await;
        let _ = invited.accept(link, &[]).await;
    }
    let chat = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(chat, viewer_id, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    let granted = owner.grant(chat, knower_id, Access::KnowExists).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");

    // The viewer, who may not list who has access, is told who may view the block and the
    // names they gave the server: not one who only knows it exists, and not their emails.
    let named = viewer
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::AccountName>>(be_block_protocol::Query::Names(chat))
        })
        .await;
    let Ok(mut named) = named else {
        panic!("a viewer could not read the names: {named:?}");
    };
    named.sort_by_key(|name| name.display_name.clone());
    assert_eq!(
        named,
        [
            be_block_protocol::AccountName {
                account: owner_id,
                display_name: "owner@example.com".to_owned(),
            },
            be_block_protocol::AccountName {
                account: viewer_id,
                display_name: "viewer@example.com".to_owned(),
            },
        ]
    );

    // Neither one who only knows the block exists nor one who cannot see it at all is told.
    for (asking, who) in [(&mut knower, "knower"), (&mut outsider, "outsider")] {
        let named = asking
            .call(|peer| {
                peer.read::<Vec<be_block_protocol::AccountName>>(be_block_protocol::Query::Names(
                    chat,
                ))
            })
            .await;
        assert!(
            refused_as(&named, ErrorCode::PermissionDenied)
                || refused_as(&named, ErrorCode::BlockNotFound),
            "the {who} was told the names: {named:?}"
        );
    }

    // An author who lost access to what holds their block is not named: the viewer makes a
    // block in a folder of the owner's, and the owner then takes the folder back.
    let folder = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(folder, viewer_id, Access::Edit).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    let made = viewer.create_block(BlockParent::Block(folder)).await;
    let mut both = vec![owner_id, viewer_id];
    both.sort();
    assert_eq!(named_accounts(&mut owner, made).await, both);
    let revoked = owner.grant(folder, viewer_id, Access::None).await;
    assert!(matches!(revoked, Ok(())), "{revoked:?}");
    assert_eq!(named_accounts(&mut owner, made).await, [owner_id]);

    harness.stop().await;
}

/// The accounts `client` is told may view `block`, in order.
async fn named_accounts(client: &mut TestClient, block: Uuid) -> Vec<Uuid> {
    let named = client
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::AccountName>>(be_block_protocol::Query::Names(block))
        })
        .await
        .expect("the names are read");
    let mut accounts: Vec<Uuid> = named.into_iter().map(|name| name.account).collect();
    accounts.sort();
    accounts
}
