use super::*;

use be_block_graph::Access;
use be_block_protocol::Unsealed;
use be_type_text::{TextBlock, TextContent};

/// A device seals a block's key to whoever its log grants, with the keys the grant names,
/// contact or not, and the grantee opens a key from a sender the organization's log names
/// (signed access 3c). A grantee who is no contact of the sealer reads, but what it pins is
/// that sender's word: its root is weak, unverified, and the key vouches for nothing.
#[tokio::test]
async fn a_grantee_who_is_not_a_contact_gets_the_key_by_the_log() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let manager = harness.member(&admin, "manager@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    assert!(manager.verified_identity(reader.account()).is_none());
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    admin
        .save(note, &TextBlock::of("for the team"), None)
        .await
        .unwrap();
    let identity = admin.verified_identity(manager.account()).unwrap();
    admin
        .share(team, manager.account(), Access::Manage, &identity)
        .await
        .unwrap();
    admin.grant(team, reader.account(), Access::View).await.unwrap();

    manager.refresh_keys(None).await.unwrap();
    let scope = manager.keys().scope_key(team).unwrap();
    let mut work = manager.key_work().await.unwrap();
    work.unsealed.retain(|unsealed| unsealed.account != reader.account());
    work.unsealed.push(Unsealed {
        key: scope,
        account: reader.account(),
    });
    manager.tend_with(work).await.unwrap();

    let device = harness.second(&reader).await;
    assert_eq!(text(&device, note).await, "for the team");
    assert!(
        matches!(device.keys().root_pin(team), crate::keys::RootPin::Weak(_)),
        "a root from a sender who is no contact is taken as verified"
    );
    assert!(
        !device.keys().trusted(scope),
        "a key a sender who is no contact sealed vouches for what it opens"
    );

    harness.stop().await;
}
