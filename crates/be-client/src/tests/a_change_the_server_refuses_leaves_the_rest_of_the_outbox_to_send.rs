use be_block::{OrganizationPolicies, WhoMay};
use be_graph::Access;
use be_protocol::ErrorCode;
use text_block_types::{TextBlock, TextContent};

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

#[tokio::test]
async fn a_change_the_server_refuses_leaves_the_rest_of_the_outbox_to_send() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    set_policies(
        &admin,
        &[OrganizationPolicies::set_workspace_creation(Some(
            WhoMay::Administrators,
        ))],
    )
    .await;
    let member = harness.member(&admin, "member@example.com").await;
    my_files(&member).await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let base = admin
        .save(note, &TextBlock::of("shared"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let public = admin.keys().contact(member.account()).unwrap();
    admin
        .share(team, member.account(), Access::Edit, &public)
        .await
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&member, &refs, &store).await;
    assert_eq!(text(&laptop, note).await, "shared");
    drop(laptop);

    let offline = harness.offline(&member, &refs, &store);
    let another = offline
        .create_block(
            Uuid::now_v7(),
            workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Another"),
        )
        .await
        .unwrap()
        .id;
    assert!(
        offline
            .save(note, &TextBlock::of("written while away"), Some(base))
            .await
            .is_err()
    );
    let written = offline.local_head(note).unwrap();

    harness.reconnect(&offline, member.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Refused {
            block: another,
            code: ErrorCode::PolicyRefused
        }),
        "{drained:?}"
    );
    assert!(
        drained.contains(&Drained::Published {
            block: note,
            head: written
        }),
        "one refused change held back the rest of the outbox: {drained:?}"
    );
    assert_eq!(
        offline.pending().operations,
        1,
        "the refused workspace was dropped rather than kept to try again"
    );
    assert_eq!(text(&admin, note).await, "written while away");

    harness.stop().await;
}
