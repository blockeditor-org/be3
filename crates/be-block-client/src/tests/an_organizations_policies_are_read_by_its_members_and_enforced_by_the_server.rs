use be_block::{
    OrganizationPolicies, OrganizationPoliciesContent, Root, Settings, SettingsContent, WhoMay,
};
use be_block_protocol::{ErrorCode, client};

use super::*;

pub(super) async fn set_policies(admin: &Peer<MemoryStore>, edits: &[be_block::Edit]) -> Uuid {
    let settings = admin.organization_settings();
    let block = admin
        .create_with_id::<OrganizationPoliciesContent>(Uuid::now_v7(), BlockParent::Block(settings))
        .await
        .unwrap();
    let mut policies = OrganizationPoliciesContent::default();
    for edit in edits {
        policies.apply(edit);
    }
    admin.save(block, &policies, None).await.unwrap();
    let mut content = SettingsContent::default();
    content.apply(&Settings::set_policy(
        OrganizationPolicies::CONTENT_TYPE,
        block,
    ));
    let head = admin.summary(settings).await.unwrap().head;
    assert!(
        admin
            .save(settings, &content, head)
            .await
            .unwrap()
            .published()
            .is_some(),
        "the administrator could not set the organization's policies"
    );
    block
}

fn refused(error: ClientError) -> ErrorCode {
    match error {
        ClientError::Refused(code, _) => code,
        other => panic!("expected a refusal, not {other}"),
    }
}

#[tokio::test]
async fn an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let settings = admin.organization_settings();
    assert!(
        admin.summary(settings).await.unwrap().access.can_manage(),
        "a new organization's administrator made its settings"
    );
    let block = set_policies(
        &admin,
        &[
            OrganizationPolicies::set_workspace_creation(Some(WhoMay::Administrators)),
            OrganizationPolicies::set_history_days(Some(30)),
        ],
    )
    .await;

    let member = harness.member(&admin, "member@example.com").await;
    let read = member.organization_policies().await.unwrap();
    assert_eq!(read.workspace_creation(), WhoMay::Administrators);
    assert_eq!(read.history_days, Some(30));
    assert!(
        !member.summary(settings).await.unwrap().access.can_edit(),
        "a member may only read the organization's settings"
    );
    let head = member.summary(settings).await.unwrap().head;
    let edited = member
        .save(settings, &SettingsContent::default(), head)
        .await
        .unwrap_err();
    assert!(
        matches!(
            edited,
            ClientError::Locked(_) | ClientError::Refused(ErrorCode::PermissionDenied, _)
        ),
        "a member edited the organization's settings: {edited}"
    );

    my_files(&member).await;
    let second = member
        .create_block(
            Uuid::now_v7(),
            be_type_workspace::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Another"),
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(refused(second), ErrorCode::PolicyRefused);
    let folder = member
        .create_block(
            Uuid::now_v7(),
            Uuid::from_u128(0xf01d),
            BlockParent::Root,
            &be_block::BlockMetadata::named("A folder at the top"),
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(
        refused(folder),
        ErrorCode::PolicyRefused,
        "a member made a workspace of another type"
    );
    let squatted = admin
        .create_block(
            be_block::my_files(member.organization(), Uuid::from_u128(0xfeed)),
            be_type_workspace::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Not mine"),
            false,
        )
        .await;
    assert!(squatted.is_ok(), "an id no account's My files uses is free");
    let squatted = member
        .create_block(
            be_block::my_files(member.organization(), admin.account()),
            be_type_workspace::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Not mine"),
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(refused(squatted), ErrorCode::PermissionDenied);
    workspace(&admin, "Team").await;
    workspace(&admin, "Projects").await;

    let image_block = admin
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    let mut head = None;
    for step in 0..4 {
        head = admin
            .save(image_block, &image("draft.png", 3_000, step + 1), head)
            .await
            .unwrap()
            .published();
    }
    assert_eq!(
        admin.history(image_block).await.unwrap().len(),
        4,
        "publishing pruned a version the organization keeps"
    );
    let current = admin.summary(image_block).await.unwrap().head;
    let mut backdated_commit = None;
    for (seed, time) in [(7, 0), (8, 1)] {
        let backdated = pseudorandom(64, seed);
        let commit = be_block_commit::CommitId::from_hash(be_block_store::Hash::of(&backdated));
        let expected = backdated_commit.or(current);
        crate::ask(client::blocks::publish(
            &admin.connection(),
            image_block,
            commit,
            expected,
            expected.into_iter().collect(),
            be_block_protocol::ObjectChange::default(),
            time,
            false,
            be_block_protocol::ReferenceChange::default(),
            Vec::new(),
            vec![backdated],
        ))
        .await
        .unwrap();
        backdated_commit.get_or_insert(commit);
    }
    let backdated_commit = backdated_commit.unwrap();
    assert!(
        admin
            .history(image_block)
            .await
            .unwrap()
            .iter()
            .any(|entry| entry.commit == backdated_commit),
        "a version that says it is old was pruned the day the server received it"
    );

    let early = pseudorandom(64, 6);
    let early_hash = be_block_store::Hash::of(&early);
    super::upload(&admin, vec![early]).await.unwrap();
    let mut policies = OrganizationPoliciesContent::default();
    policies.apply(&OrganizationPolicies::set_storage_quota(Some(1)));
    policies.apply(&OrganizationPolicies::set_history_days(Some(30)));
    let head = admin.summary(block).await.unwrap().head;
    admin.save(block, &policies, head).await.unwrap();
    let over = super::upload(&admin, vec![pseudorandom(64, 5)])
        .await
        .unwrap_err();
    assert_eq!(refused(over), ErrorCode::OverQuota);
    let held = crate::ask(client::objects::hold_objects(
        &admin.connection(),
        image_block,
        vec![early_hash],
        be_block_protocol::Hold::Lasting,
    ))
    .await
    .unwrap_err();
    assert_eq!(
        refused(held),
        ErrorCode::OverQuota,
        "an object uploaded before the quota was held past it"
    );
    let current = admin.summary(image_block).await.unwrap().head;
    let published = crate::ask(client::blocks::publish(
        &admin.connection(),
        image_block,
        be_block_commit::CommitId::from_hash(early_hash),
        current,
        current.into_iter().collect(),
        be_block_protocol::ObjectChange::default(),
        0,
        false,
        be_block_protocol::ReferenceChange::default(),
        Vec::new(),
        Vec::new(),
    ))
    .await
    .unwrap_err();
    assert_eq!(
        refused(published),
        ErrorCode::OverQuota,
        "an object uploaded before the quota was published past it"
    );

    admin
        .set_parent(image_block, BlockParent::Detached)
        .await
        .unwrap();
    assert!(
        !admin
            .collect_detached()
            .await
            .unwrap()
            .contains(&image_block),
        "a deleted block was collected with versions the organization keeps"
    );

    harness.stop().await;
}
