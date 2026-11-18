use be_block::{LinkSharing, OrganizationPolicies};
use be_block_keys::Admits;
use be_block_protocol::{ErrorCode, OrganizationRole};
use be_type_text::{TextBlock, TextContent};

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// A link that admits members refuses an account from another organization. One that admits
/// anyone, which the organization's policy must allow, makes such an account a guest: it reads
/// what the link shares, makes nothing there, and leaves once the link is revoked.
#[tokio::test]
async fn a_share_link_admits_only_members_unless_the_policy_lets_it_admit_guests() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let outsider = harness.owner("outsider@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    admin
        .save(note, &TextBlock::of("for guests"), None)
        .await
        .unwrap();

    let members = admin
        .make_share_link(team, Access::Edit, Admits::Members)
        .await
        .unwrap();
    let refused = crate::share_links::read_share_link(&outsider.connection(), &members)
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused,
            ClientError::Refused(ErrorCode::PermissionDenied, _)
        ),
        "{refused}"
    );
    let refused = admin
        .make_share_link(team, Access::View, Admits::Anyone)
        .await
        .unwrap_err();
    assert!(
        matches!(refused, ClientError::Refused(ErrorCode::PolicyRefused, _)),
        "the default policy admits members alone: {refused}"
    );

    set_policies(
        &admin,
        &[OrganizationPolicies::set_link_sharing(Some(
            LinkSharing::Anyone,
        ))],
    )
    .await;
    let anyone = admin
        .make_share_link(team, Access::View, Admits::Anyone)
        .await
        .unwrap();
    let opened = crate::share_links::open_share_link(
        &outsider.connection(),
        outsider.keys().identity(),
        outsider.account(),
        &anyone,
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
    assert_eq!(text(&guest, note).await, "for guests");
    let made = guest.create::<TextContent>(BlockParent::Block(team)).await;
    assert!(made.is_err(), "a guest makes no blocks");

    let rows = admin.share_links(team).await.unwrap();
    let (row, _) = rows
        .iter()
        .find(|(row, _)| row.id == crate::share_links::link_id(&anyone))
        .unwrap();
    admin.revoke_share_link(team, row).await.unwrap();
    let left: i64 = harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM memberships WHERE organization_id = ?1 AND account_id = ?2",
            [
                admin.organization().to_string(),
                outsider.account().to_string(),
            ],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        left, 0,
        "a guest with no grant left leaves the organization"
    );

    harness.stop().await;
}
