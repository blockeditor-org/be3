use super::*;
use be_graph::Access;

#[tokio::test]
async fn a_block_made_offline_takes_the_access_the_server_will_give_it() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let team = workspace(&admin, "Team").await;
    let public = admin.keys().contact(member.account()).unwrap();
    admin
        .share(team, member.account(), Access::Edit, &public)
        .await
        .unwrap();
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&member, &refs, &store).await;
    assert_eq!(laptop.summary(team).await.unwrap().access, Access::Edit);
    drop(laptop);

    let offline = harness.offline(&member, &refs, &store);
    let settings = offline
        .create::<be_block::SettingsContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let tags = offline
        .create_block(
            Uuid::now_v7(),
            be_graph::TAGS_TYPE,
            BlockParent::Block(team),
            &be_block::BlockMetadata::default(),
        )
        .await
        .unwrap();
    let tag = offline
        .create::<text_block_types::TextContent>(BlockParent::Block(tags.id))
        .await
        .unwrap();
    let note = offline
        .create::<text_block_types::TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    assert_eq!(
        offline.summary(settings).await.unwrap().access,
        Access::View
    );
    assert_eq!(tags.access, Access::Edit);
    assert_eq!(offline.summary(tag).await.unwrap().access, Access::Edit);
    assert_eq!(offline.summary(note).await.unwrap().access, Access::Manage);
    harness.stop().await;
}
