use be_block::{LinkSharing, OrganizationPolicies};
use be_block_keys::Admits;
use be_type_text::{TextBlock, TextContent};

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// A guest with an Edit link is served the organization's log as a proof path that names no
/// member, reads none of its settings, and leaves the organization when it gives up its last
/// grant itself.
#[tokio::test]
async fn a_guest_sees_no_members_or_settings_and_leaves_with_its_last_grant() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let outsider = harness.owner("outsider@example.com").await;
    set_policies(
        &admin,
        &[OrganizationPolicies::set_link_sharing(Some(
            LinkSharing::Anyone,
        ))],
    )
    .await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    admin
        .save(note, &TextBlock::of("for a guest"), None)
        .await
        .unwrap();
    let code = admin
        .make_share_link(team, Access::Edit, Admits::Anyone)
        .await
        .unwrap();
    crate::share_links::open_share_link(
        &outsider.connection(),
        outsider.keys().identity(),
        outsider.account(),
        &code,
    )
    .await
    .unwrap();
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
    assert_eq!(text(&guest, note).await, "for a guest");

    let served = be_block_protocol::read::<be_block_protocol::AccessLogPage, _>(
        &guest.connection(),
        Some(admin.organization()),
        be_block_protocol::Query::Log {
            log: None,
            held: None,
        },
    )
    .await
    .unwrap()
    .unwrap();
    let steps: Vec<Step> = served
        .entries
        .iter()
        .map(|bytes| Step::decode(bytes).unwrap())
        .collect();
    assert!(
        steps.iter().any(|step| matches!(step, Step::Reduced(_))),
        "a guest with Edit is served the organization's log as a proof path"
    );
    assert!(
        !steps.iter().any(|step| matches!(
            step,
            Step::Entry(entry) if matches!(&entry.record, Record::Member(named) if named.account == member.account().into_bytes())
        )),
        "the path names no member to a guest"
    );
    assert!(
        !guest
            .summary(admin.organization_settings())
            .await
            .is_ok_and(|summary| summary.access.can_view()),
        "a guest reads none of the organization's settings"
    );

    guest
        .grant(team, guest.account(), Access::None)
        .await
        .unwrap();
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
    assert_eq!(left, 0, "a guest that gives up its last grant leaves");

    harness.stop().await;
}
