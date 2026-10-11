use std::sync::Arc;

use be_block::text::{TextBlock, TextContent};
use be_client::Live;

use super::*;

#[tokio::test]
async fn a_page_open_elsewhere_takes_the_change_through_its_session() {
    let harness = Harness::start().await;
    let owner = harness.owner().await;
    let token = owner.token().to_owned();
    let workspace = owner.workspace();
    let tracker = Tracker::new(owner);
    run(&tracker, "new-database Issues --prefix BE")
        .await
        .unwrap();
    let database = tracker.find_database("Issues").await.unwrap();
    let made = tracker
        .create(database, "Fix the login", &[], Body::Replace("one".into()))
        .await
        .unwrap();
    let page = made.page.unwrap();

    let editor = Arc::new(
        Peer::connect(
            PeerConfig::new(
                be_client::socket_url(&harness.url),
                ContentKey::from_bytes(KEY),
                Credentials::Token(token),
            )
            .workspace(Some(workspace)),
            MemoryStore::new(),
        )
        .await
        .unwrap(),
    );
    let mut live = Live::<MemoryStore, TextContent>::join(editor, page)
        .await
        .unwrap();
    assert!(live.is_owner());
    let typed = TextBlock::insert(live.content(), live.client(), 3, b" typed").unwrap();
    live.edit(typed).await.unwrap();
    let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stopped => return live,
                waited = live.wait() => { waited.unwrap(); }
            }
        }
    });

    let changed = tracker
        .update(&made.id, &[], Body::Append("appended".into()))
        .await
        .unwrap();
    stop.send(()).unwrap();
    let live = serving.await.unwrap();

    assert_eq!(changed.body.as_deref(), Some("one typed\nappended"));
    assert_eq!(TextBlock::text(live.content()), "one typed\nappended");
    harness.stop().await;
}
