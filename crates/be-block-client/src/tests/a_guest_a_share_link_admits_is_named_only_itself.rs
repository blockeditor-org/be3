use be_block::{LinkSharing, OrganizationPolicies};
use be_block_keys::Admits;
use be_block_protocol::OrganizationRole;
use be_type_text::TextContent;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// A guest learns no member of the organization: asked who may view a block a link shared
/// with it, the server names the guest alone, while a member asking is told everyone.
#[tokio::test]
async fn a_guest_a_share_link_admits_is_named_only_itself() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let outsider = harness.owner("outsider@example.com").await;
    let team = workspace(&admin, "Team").await;
    let chat = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    set_policies(
        &admin,
        &[OrganizationPolicies::set_link_sharing(Some(
            LinkSharing::Anyone,
        ))],
    )
    .await;
    let link = admin
        .make_share_link(team, Access::View, Admits::Anyone)
        .await
        .unwrap();
    let (opened, _) = crate::share_links::open_share_link(
        &outsider.connection(),
        outsider.keys().identity(),
        outsider.account(),
        &link,
        None,
    )
    .await
    .unwrap();
    assert!(opened.guest, "an outsider joins as a guest");
    let guest = Peer::connect(
        PeerConfig::new(
            &harness.url,
            outsider.keys().identity().clone(),
            Credentials::Token(outsider.token()),
        )
        .organization(Some(admin.organization()))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    assert_eq!(guest.role(), OrganizationRole::Guest);

    let accounts = |names: Vec<be_block_protocol::AccountName>| {
        let mut accounts: Vec<Uuid> = names.into_iter().map(|name| name.account).collect();
        accounts.sort();
        accounts
    };
    let mut everyone = vec![admin.account(), outsider.account()];
    everyone.sort();
    assert_eq!(
        accounts(admin.names(chat).await.unwrap()),
        everyone,
        "a member is told the guest too"
    );
    assert_eq!(
        accounts(guest.names(chat).await.unwrap()),
        [outsider.account()],
        "a guest is named only itself, never a member"
    );

    harness.stop().await;
}
