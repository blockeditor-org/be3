use be_block::{LinkSharing, OrganizationPolicies};
use be_type_text::{TextBlock, TextContent};

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// An invitation follows the organization's outside sharing, not its link sharing: with share
/// links off and outside sharing allowed, one is made, and its share half opens for the person
/// invited.
#[tokio::test]
async fn an_invitation_is_made_and_opens_where_share_links_are_off_but_the_organization_shares_outside()
 {
    let harness = Harness::start().await;
    let maker = harness.owner("maker@example.com").await;
    let invited = harness.owner("invited@example.com").await;
    set_policies(
        &maker,
        &[OrganizationPolicies::set_link_sharing(Some(LinkSharing::Off))],
    )
    .await;
    let team = workspace(&maker, "Team").await;
    let note = maker
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    maker
        .save(note, &TextBlock::of("invited anyway"), None)
        .await
        .unwrap();
    let card = crate::contacts::card(
        maker.account(),
        maker.keys().identity(),
        "maker@example.com",
        "maker@example.com",
    );

    let code = maker.make_invitation(&card, team, Access::View).await.unwrap();
    crate::contacts::accept_link(
        &invited.connection(),
        invited.keys().identity(),
        &crate::contacts::card(
            invited.account(),
            invited.keys().identity(),
            "invited@example.com",
            "invited@example.com",
        ),
        &code,
        crate::contacts::Expected::Contact,
        &Default::default(),
    )
    .await
    .unwrap();
    let (opened, _) = crate::share_links::open_share_link(
        &invited.connection(),
        invited.keys().identity(),
        invited.account(),
        &code,
        None,
    )
    .await
    .unwrap();
    assert_eq!((opened.block, opened.access), (team, Access::View));
    let guest = Peer::connect(
        PeerConfig::new(
            &harness.url,
            invited.keys().identity().clone(),
            Credentials::Token(invited.token()),
        )
        .organization(Some(maker.organization()))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    assert_eq!(text(&guest, note).await, "invited anyway");

    harness.stop().await;
}
