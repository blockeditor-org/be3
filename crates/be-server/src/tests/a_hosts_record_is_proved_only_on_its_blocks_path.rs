use be_block_graph::Access;
use be_block_keys::{Record, Step};

use super::*;

/// The accounts the whole grants of `block`'s log, as served to `client`, name.
async fn granted(client: &mut TestClient, block: Uuid) -> Vec<Uuid> {
    let served = client
        .access_log(Some(block))
        .await
        .expect("reading the log failed");
    decode_steps(served)
        .iter()
        .filter_map(Step::entry)
        .filter_map(|entry| match &entry.record {
            Record::Grant(grant) => Some(Uuid::from_bytes(grant.grantee)),
            _ => None,
        })
        .collect()
}

/// A member of a hosted block's session is served the lease holder's records on that block's
/// path (guides/blocks/hosted.md), to check its right to host there, and nowhere else: the path
/// of an unrelated block the member reads still reduces the host's grant there.
#[tokio::test]
async fn a_hosts_record_is_proved_only_on_its_blocks_path() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let mut editor = harness.member("editor@example.com").await;
    let mut player = harness.member("player@example.com").await;
    for other in [&mut editor, &mut player] {
        other.personal().await;
        let link = owner.link().await;
        let accepted = other.accept(link, &[]).await;
        assert!(accepted.is_ok(), "{accepted:?}");
    }
    let games = owner.create_block(BlockParent::Root).await;
    let world = owner.create_block(BlockParent::Block(games)).await;
    let elsewhere = owner.create_block(BlockParent::Block(games)).await;
    for (block, account, access) in [
        (world, editor.id(), Access::Edit),
        (elsewhere, editor.id(), Access::Edit),
        (world, player.id(), Access::Join),
        (elsewhere, player.id(), Access::View),
    ] {
        let granted = owner.grant(block, account, access).await;
        assert!(granted.is_ok(), "{granted:?}");
    }

    let before = granted(&mut player, world).await;
    assert!(
        !before.contains(&editor.id()),
        "the editor's grant was proved with no session"
    );
    editor.join_as_member(world).await.expect("joining failed");
    editor.claim(world).await.expect("claiming failed");
    let joined = player.join_as_member(world).await.expect("joining failed");
    assert_eq!(joined.host.map(|host| host.account), Some(editor.id()));

    assert!(
        granted(&mut player, world).await.contains(&editor.id()),
        "the host's grant on the hosted block is proved to its member"
    );
    assert!(
        !granted(&mut player, elsewhere).await.contains(&editor.id()),
        "the host's grant on an unrelated block was proved to a member of its session"
    );

    harness.stop().await;
}
