use be_block_graph::Access;

use super::*;
use crate::client::JOIN_PAUSE;

/// A ban made between a join's first check of it and the connection's registering in the
/// session (`Client::join`) is caught by the check after registering: the join is refused and
/// the connection leaves again, for a participant and a Join member alike
/// (guides/blocks/hosted.md, "Hosting").
#[tokio::test]
async fn a_ban_made_while_a_join_registers_takes_it_out_again() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let mut viewer = harness.member("viewer@example.com").await;
    let mut player = harness.member("player@example.com").await;
    for other in [&mut viewer, &mut player] {
        other.personal().await;
        let link = owner.link().await;
        let accepted = other.accept(link, &[]).await;
        assert!(accepted.is_ok(), "{accepted:?}");
    }
    let games = owner.create_block(BlockParent::Root).await;
    let world = owner.create_block(BlockParent::Block(games)).await;
    for (account, access) in [(viewer.id(), Access::View), (player.id(), Access::Join)] {
        let granted = owner.grant(world, account, access).await;
        assert!(granted.is_ok(), "{granted:?}");
    }
    owner.join_as_member(world).await.expect("joining failed");
    owner.claim(world).await.expect("claiming failed");
    let key = world.as_u128();

    for (joiner, kind) in [(&mut viewer, "a participant"), (&mut player, "a Join member")] {
        let account = joiner.id();
        be_lib_test_pause::hold(JOIN_PAUSE, key);
        let ban = async {
            be_lib_test_pause::reached(JOIN_PAUSE, key, 1).await;
            let banned = owner.ban(world, account).await;
            be_lib_test_pause::release(JOIN_PAUSE, key);
            banned
        };
        let (joined, banned) =
            futures_util::future::join(joiner.join_as_member(world), ban).await;
        assert!(banned.is_ok(), "{banned:?}");
        assert!(
            refused_as(&joined, ErrorCode::PermissionDenied),
            "{kind} banned as it joined stays out: {joined:?}"
        );
        let sent = joiner.send_to_host(world, None, b"here".to_vec()).await;
        assert!(
            refused_as(&sent, ErrorCode::PermissionDenied),
            "{kind} is in no session to send its host anything in: {sent:?}"
        );
    }
}
