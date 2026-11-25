use super::*;

use be_block_graph::Access;
use be_block_keys::Level;

/// A host admits a member only while the block's log grants its account Join or above now, with
/// the identity key it speaks with (guides/blocks/hosted.md, "The channel"): one revoked since it
/// began the handshake is refused, and so is a first message with another account's key.
#[tokio::test]
async fn a_joiner_whose_grant_was_revoked_is_refused_by_the_host() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let player = harness.member(&owner, "player@example.com").await;
    let other = harness.member(&owner, "other@example.com").await;
    for member in [&player, &other] {
        member.refresh_contacts().await.unwrap();
    }
    let games = workspace(&owner, "Games").await;
    let world = owner
        .create::<TextContent>(BlockParent::Block(games))
        .await
        .unwrap();
    for member in [&player, &other] {
        owner
            .grant(world, member.account(), Access::Join)
            .await
            .unwrap();
    }

    let (joining, first) = player.join_host(world, owner.account()).await.unwrap();
    let (admitting, answer) = owner.admit(world, &first).await.unwrap();
    let mut channel = joining.finish(&answer).unwrap();
    let (admitted, _) = admitting.confirm(&channel.seal(b"hello")).unwrap();
    assert_eq!(
        (admitted.account, admitted.level),
        (player.account(), Level::Join),
        "a member granted Join is admitted"
    );

    // Another member names the player's account but speaks with its own key.
    let (head, _) = first.split_at(32);
    let host = owner.keys().identity().public();
    let prologue =
        be_block_keys::noise::hosted_prologue(world.into_bytes(), head.try_into().unwrap());
    let (_, posing) = be_block_keys::noise::Initiator::start(
        other.keys().identity(),
        &host,
        &prologue,
        player.account().as_bytes(),
    );
    let posing = [head, &posing].concat();
    assert!(
        owner.admit(world, &posing).await.is_err(),
        "a member was admitted as another account"
    );
    let (_, theirs) = other.join_host(world, owner.account()).await.unwrap();
    let (admitting, _) = owner.admit(world, &theirs).await.unwrap();
    assert_eq!(admitting.account(), other.account());

    let (_, before) = player.join_host(world, owner.account()).await.unwrap();
    owner
        .grant(world, player.account(), Access::None)
        .await
        .unwrap();
    let refused = owner.admit(world, &before).await;
    assert!(
        matches!(
            &refused,
            Err(ClientError::Refused(
                be_block_protocol::ErrorCode::PermissionDenied,
                _
            ))
        ),
        "a member revoked since was admitted: {refused:?}"
    );
    assert!(
        player.join_host(world, owner.account()).await.is_err(),
        "a revoked member read the block's log"
    );
    assert!(owner.admit(world, &theirs).await.is_ok());

    harness.stop().await;
}
