use super::a_share_link_grants_view_to_an_account_no_one_granted::personal;
use super::*;

use be_block_keys::{Admits, Authority, Entry, Level, LogKind, Record, RecoveryPhrase, ShareLink};
use be_type_text::TextContent;

/// A server that serves a Join link's opener a log of its own making (a root it signed, naming
/// the block's real owner, and a link record naming the real link's key, which it can copy)
/// gets the opener's grant signed over that log's head, but no more: the link handed the real
/// root, sealed with its code, so opening is refused as forged and so is joining a host by that
/// log.
#[tokio::test]
async fn a_join_link_opener_refuses_a_log_the_server_makes_up() {
    let harness = Harness::start().await;
    let owner = personal(&harness, "owner@example.com").await;
    let player = personal(&harness, "player@example.com").await;
    let games = workspace(&owner, "Games").await;
    let world = owner
        .create::<TextContent>(BlockParent::Block(games))
        .await
        .unwrap();
    let code = owner
        .make_share_link(world, Access::Join, Admits::Members)
        .await
        .unwrap();

    // The server's own log for the block, rooted at keys it holds.
    let impostor = RecoveryPhrase::generate().secret();
    let creator = owner.account().into_bytes();
    let root = Entry::root(&impostor, creator, LogKind::Block, world.into_bytes());
    let link = Entry::sign(
        &impostor,
        creator,
        world.into_bytes(),
        Some(root.hash()),
        false,
        Record::ShareLink(ShareLink {
            key: code.share_key(world.into_bytes()),
            level: Level::Join,
            admits: Admits::Members,
            open: true,
            authority: Authority::Log,
        }),
    );
    let database = harness.database();
    database
        .execute(
            "DELETE FROM access_log WHERE organization_id = ?1 AND block_id = ?2",
            [owner.organization().to_string(), world.to_string()],
        )
        .unwrap();
    for (position, entry) in [root, link].iter().enumerate() {
        database
            .execute(
                "INSERT INTO access_log (organization_id, block_id, position, entry)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    owner.organization().to_string(),
                    world.to_string(),
                    position as i64,
                    entry.encode()
                ],
            )
            .unwrap();
    }

    let opened = crate::share_links::open_share_link(
        &player.connection(),
        player.keys().identity(),
        player.account(),
        &code,
        None,
    )
    .await;
    assert!(
        matches!(opened, Err(ClientError::Forged(block)) if block == world),
        "a log not rooted where the link says was taken: {opened:?}"
    );
    let device = harness.second(&player).await;
    let joined = device.join_host(world, owner.account()).await;
    assert!(
        matches!(joined, Err(ClientError::Forged(_))),
        "a Join member checked a host by a log the server made up"
    );

    harness.stop().await;
}
