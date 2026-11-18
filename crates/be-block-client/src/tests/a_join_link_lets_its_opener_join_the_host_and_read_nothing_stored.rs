use super::a_share_link_grants_view_to_an_account_no_one_granted::personal;
use super::*;

use be_block_keys::{Admits, Level};
use be_type_text::{TextBlock, TextContent};

/// A Join link carries no key: its opener, whom no contact granted anything, reads nothing
/// stored, and joins whoever hosts, checking the host by the log its own grant fixes.
#[tokio::test]
async fn a_join_link_lets_its_opener_join_the_host_and_read_nothing_stored() {
    let harness = Harness::start().await;
    let owner = personal(&harness, "owner@example.com").await;
    let player = personal(&harness, "player@example.com").await;
    let games = workspace(&owner, "Games").await;
    let world = owner
        .create::<TextContent>(BlockParent::Block(games))
        .await
        .unwrap();
    owner
        .save(world, &TextBlock::of("the world's secrets"), None)
        .await
        .unwrap();
    let code = owner
        .make_share_link(world, Access::Join, Admits::Members)
        .await
        .unwrap();

    let (preview, handed) =
        crate::share_links::preview_share_link(&player.connection(), &code)
            .await
            .unwrap();
    assert_eq!(preview.access, Access::Join);
    assert!(handed.scope.is_none(), "a Join link carries no key");
    crate::share_links::open_share_link(
        &player.connection(),
        player.keys().identity(),
        player.account(),
        &code,
        None,
    )
    .await
    .unwrap();
    assert_eq!(player.summary(world).await.unwrap().access, Access::Join);
    assert!(
        player.open::<TextContent>(world).await.is_err(),
        "a Join member reads nothing stored"
    );

    let (joining, first) = player.join_host(world, owner.account()).await.unwrap();
    let (admitting, answer) = owner.admit(world, &first).await.unwrap();
    let mut channel = joining.finish(&answer).unwrap();
    let (admitted, _) = admitting.confirm(&channel.seal(b"hello")).unwrap();
    assert_eq!(
        (admitted.account, admitted.level),
        (player.account(), Level::Join),
        "the host admits the link's opener"
    );

    // Revoked, the link's grant falls: its opener joins no more, and forgets the root it handed.
    let listed = owner.share_links(world).await.unwrap();
    let (row, kept_code) = listed.first().expect("the owner lists the link it made");
    assert!(kept_code.is_some(), "its maker keeps the code");
    owner.revoke_share_link(world, row).await.unwrap();
    assert!(player.join_host(world, owner.account()).await.is_err());
    assert!(
        player
            .kept_shares()
            .await
            .unwrap()
            .iter()
            .all(|(_, kept)| kept.block != world),
        "the opener forgets a link whose grant fell"
    );

    harness.stop().await;
}
