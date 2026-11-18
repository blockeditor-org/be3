use super::*;

use be_block_graph::Access;
use be_block_keys::Level;

/// A ban record in a hosted block's log (plans/hosted_blocks.md, "Hosting"), which any editor
/// signs, bars its account from joining: the server refuses it the session and the host refuses
/// its handshake. A grant signed after it lifts it.
#[tokio::test]
async fn a_banned_member_is_refused_until_a_grant_lifts_the_ban() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let editor = harness.member(&owner, "editor@example.com").await;
    let player = harness.member(&owner, "player@example.com").await;
    player.refresh_contacts().await.unwrap();
    let games = workspace(&owner, "Games").await;
    let world = owner
        .create::<TextContent>(BlockParent::Block(games))
        .await
        .unwrap();
    let identity = owner.verified_identity(editor.account()).unwrap();
    owner
        .share(world, editor.account(), Access::Edit, &identity)
        .await
        .unwrap();
    owner
        .grant(world, player.account(), Access::Join)
        .await
        .unwrap();
    editor.refresh_keys(None).await.unwrap();

    let (_, before) = player.join_host(world, owner.account()).await.unwrap();
    editor.ban(world, player.account()).await.unwrap();
    let joined = player
        .connection()
        .join(player.organization(), world, BUILD)
        .await;
    assert!(
        matches!(
            &joined,
            Err(ClientError::Refused(be_block_protocol::ErrorCode::PermissionDenied, _))
        ),
        "the server let a banned account join: {joined:?}"
    );
    let admitted = owner.admit(world, &before).await;
    assert!(admitted.is_err(), "the host admitted a banned account");

    owner
        .grant(world, player.account(), Access::Join)
        .await
        .unwrap();
    player
        .connection()
        .join(player.organization(), world, BUILD)
        .await
        .expect("a grant after the ban lifts it");
    let (_, after) = player.join_host(world, owner.account()).await.unwrap();
    let (admitting, _) = owner.admit(world, &after).await.unwrap();
    assert_eq!(admitting.level(), Level::Join);

    harness.stop().await;
}
