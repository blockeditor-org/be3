use be_block::{
    OrganizationPolicies, OrganizationPoliciesContent, Root, Settings, SettingsContent, WhoMay,
};
use be_protocol::{ErrorCode, client};

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
            workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Another"),
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
            workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Not mine"),
        )
        .await;
    assert!(squatted.is_ok(), "an id no account's My files uses is free");
    let squatted = member
        .create_block(
            be_block::my_files(member.organization(), admin.account()),
            workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Not mine"),
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
    let history = admin.history(image_block).await.unwrap();
    let thinning = RetentionPolicy {
        keep_all_within: 0,
        buckets: vec![be_commit::retention::Bucket {
            until_age: i64::MAX,
            interval: 10 * MINUTE,
        }],
    };
    assert_eq!(admin.prune(image_block, &thinning).await.unwrap(), 0);
    assert_eq!(
        admin.history(image_block).await.unwrap().len(),
        history.len()
    );
    let oldest = history.last().unwrap().commit;
    let pruned = crate::ask(client::blocks::prune_history(
        &admin.connection(),
        image_block,
        vec![oldest],
    ))
    .await
    .unwrap_err();
    assert_eq!(refused(pruned), ErrorCode::PolicyRefused);
    let backdated = pseudorandom(64, 7);
    let backdated_commit = be_commit::CommitId::from_hash(be_store::Hash::of(&backdated));
    let current = admin.summary(image_block).await.unwrap().head;
    crate::ask(client::blocks::publish(
        &admin.connection(),
        image_block,
        backdated_commit,
        current,
        current.into_iter().collect(),
        Vec::new(),
        0,
        false,
        Vec::new(),
        Vec::new(),
        vec![backdated],
    ))
    .await
    .unwrap();
    let pruned = crate::ask(client::blocks::prune_history(
        &admin.connection(),
        image_block,
        vec![backdated_commit],
    ))
    .await
    .unwrap_err();
    assert_eq!(
        refused(pruned),
        ErrorCode::PolicyRefused,
        "a version that says it is old was pruned the day the server received it"
    );

    let early = pseudorandom(64, 6);
    let early_hash = be_store::Hash::of(&early);
    crate::ask(client::objects::put_objects(
        &admin.connection(),
        vec![early],
    ))
    .await
    .unwrap();
    let mut policies = OrganizationPoliciesContent::default();
    policies.apply(&OrganizationPolicies::set_storage_quota(Some(1)));
    policies.apply(&OrganizationPolicies::set_history_days(Some(30)));
    let head = admin.summary(block).await.unwrap().head;
    admin.save(block, &policies, head).await.unwrap();
    let over = crate::ask(client::objects::put_objects(
        &admin.connection(),
        vec![pseudorandom(64, 5)],
    ))
    .await
    .unwrap_err();
    assert_eq!(refused(over), ErrorCode::OverQuota);
    let held = crate::ask(client::objects::hold_objects(
        &admin.connection(),
        image_block,
        vec![early_hash],
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
        be_commit::CommitId::from_hash(early_hash),
        current,
        current.into_iter().collect(),
        Vec::new(),
        0,
        false,
        Vec::new(),
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

    admin.delete(image_block).await.unwrap();
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
