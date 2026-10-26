use super::*;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_page_saved_elsewhere_keeps_both_changes() {
    let harness = Harness::start().await;
    let owner = harness.owner().await;
    let token = owner.token().to_owned();
    let identity = owner.keys().identity().clone();
    let organization = owner.organization();
    let tracker = Tracker::new(owner, sources());
    run(&tracker, "new-database Issues --prefix BE")
        .await
        .unwrap();
    let database = tracker.find_database("Issues").await.unwrap();
    let made = tracker
        .create(database, "Fix the login", &[], Body::Replace("one".into()))
        .await
        .unwrap();
    let page = made.page.unwrap();

    let editor = Peer::connect(
        PeerConfig::new(
            be_client::socket_url(&harness.url),
            identity,
            Credentials::Token(token),
        )
        .organization(Some(organization)),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let mut text = editor.open::<TextContent>(page).await.unwrap().unwrap();
    let head = editor.local_head(page);
    let typed = TextBlock::insert(&text, 7, 3, b" typed").unwrap();
    text.apply(&typed);
    editor.save(page, &text, head).await.unwrap();

    let changed = tracker
        .update(database, &made.id, &[], Body::Append("appended".into()))
        .await
        .unwrap();

    assert_eq!(changed.body.as_deref(), Some("one typed\nappended"));
    harness.stop().await;
}
