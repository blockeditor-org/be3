use be_block::{LinkSharing, OrganizationPolicies};
use be_block_keys::Admits;
use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// A share link that admits one person, which the policy must allow as it allows a link for
/// anyone, opens for the first account that opens it, reads for that one, and for no one else:
/// the server refuses a second opener, as it refuses one the link does not admit.
#[tokio::test]
async fn a_share_link_admitting_one_opens_for_its_first_opener_alone() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let invited = harness.owner("invited@example.com").await;
    let other = harness.owner("other@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    admin
        .save(note, &TextBlock::of("for one"), None)
        .await
        .unwrap();

    let refused = admin
        .make_share_link(team, Access::View, Admits::One)
        .await
        .unwrap_err();
    assert!(
        matches!(refused, ClientError::Refused(ErrorCode::PolicyRefused, _)),
        "a link for one person reaches outside, as one for anyone does: {refused}"
    );
    set_policies(
        &admin,
        &[OrganizationPolicies::set_link_sharing(Some(
            LinkSharing::Anyone,
        ))],
    )
    .await;
    let one = admin
        .make_share_link(team, Access::View, Admits::One)
        .await
        .unwrap();

    let (opened, _) = open(&invited, &one).await.unwrap();
    assert!(opened.guest, "the first opener joins as a guest");
    let read = crate::share_links::read_share_link(&invited.connection(), &one).await;
    assert!(
        read.is_ok(),
        "the one it admitted may still read it: {read:?}"
    );

    let refused = crate::share_links::read_share_link(&other.connection(), &one)
        .await
        .unwrap_err();
    assert!(
        matches!(refused, ClientError::Refused(ErrorCode::PermissionDenied, _)),
        "a second person may not read it: {refused}"
    );
    let refused = open(&other, &one).await.unwrap_err();
    assert!(
        matches!(refused, ClientError::Refused(ErrorCode::PermissionDenied, _)),
        "nor open it: {refused}"
    );

    let rows = admin.share_links(team).await.unwrap();
    assert!(
        rows.iter()
            .any(|(row, _)| row.admits == be_block_protocol::LinkAdmits::One),
        "its managers see whom it admits: {rows:?}"
    );

    harness.stop().await;
}

async fn open(
    account: &Peer<MemoryStore>,
    code: &be_block_keys::InviteCode,
) -> Result<(be_block_protocol::ShareLink, crate::share_links::Handed), ClientError> {
    crate::share_links::open_share_link(
        &account.connection(),
        account.keys().identity(),
        account.account(),
        code,
        None,
    )
    .await
}
