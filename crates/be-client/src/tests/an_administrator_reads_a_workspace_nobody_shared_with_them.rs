use super::*;

use be_protocol::OrganizationRole;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn an_administrator_reads_a_workspace_nobody_shared_with_them() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let first = admin.key_work().await.unwrap().escrows.last().cloned();
    assert!(first.is_some(), "a company organization has no escrow key");
    let member = harness.member(&admin, "member@example.com").await;
    let private = workspace(&member, "Private").await;
    let note = member
        .create::<TextContent>(BlockParent::Block(private))
        .await
        .unwrap();
    member
        .save(note, &TextBlock::of("only mine"), None)
        .await
        .unwrap();

    let laptop = harness.second(&admin).await;
    assert_eq!(text(&laptop, note).await, "only mine");

    let deputy = harness.member(&admin, "deputy@example.com").await;
    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    let promoted = harness.second(&deputy).await;
    assert_eq!(text(&promoted, note).await, "only mine");

    admin
        .set_role(deputy.account(), OrganizationRole::Editor)
        .await
        .unwrap();
    assert!(admin.key_work().await.unwrap().escrow_rotation);
    admin.tend().await.unwrap();
    let work = admin.key_work().await.unwrap();
    assert!(!work.escrow_rotation);
    let latest = work.escrows.last().cloned().unwrap();
    assert!(
        latest.epoch > first.clone().unwrap().epoch,
        "demoting an administrator kept the escrow key"
    );
    let rejoined = harness.second(&member).await;
    assert_eq!(rejoined.keys().pinned(), Some(latest.clone()));
    assert_eq!(
        rejoined.keys().escrow(),
        Some(latest),
        "a member did not follow the escrow key from the one it pinned to the next"
    );
    assert_eq!(rejoined.keys().escrow_warning(), None);
    let demoted = harness.second(&deputy).await;
    assert!(demoted.summary(note).await.is_err());
    assert_eq!(text(&harness.second(&admin).await, note).await, "only mine");

    harness.stop().await;
}
