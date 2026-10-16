use be_block::{BlockContent, FolderContent, TextContent};
use be_graph::BlockParent;

use super::*;

#[tokio::test]
async fn a_tracker_downloaded_as_a_block_file_uploads_and_reads_back() {
    let server = Server::start().await;
    let tracker = server.owner().await;
    let blocks = be_client::transfer::decode(TRACKER).unwrap();
    let uploaded = tracker
        .upload(blocks, BlockParent::Root, None)
        .await
        .unwrap();
    assert_eq!(uploaded.blocks, 6);

    let index = tracker.index().await.unwrap();
    let notes = index.get(uploaded.top).unwrap().clone();
    assert_eq!(notes.name.as_deref(), Some(scene::NOTES));
    let folder = tracker
        .read::<FolderContent>(notes.id)
        .await
        .unwrap()
        .unwrap();
    let children = folder.root().blocks();
    assert_eq!(children.len(), 2);
    let shopping = children
        .iter()
        .copied()
        .find(|child| index.get(*child).unwrap().block_type == TextContent::CONTENT_TYPE)
        .unwrap();
    assert_eq!(tracker.text(shopping).await.unwrap(), scene::SHOPPING[2]);

    let database = tracker.find_database(scene::DATABASE).await.unwrap();
    let rows = tracker.list(database, &[]).await.unwrap();
    let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, [scene::FIRST_ROW, scene::SECOND_ROW]);
    assert!(rows[0].id.starts_with(&format!("{}-", scene::PREFIX)));
    assert_eq!(rows[0].field("Status").unwrap().text, "Done");
    assert_eq!(rows[1].field("Status").unwrap().text, "In progress");
    assert_eq!(
        rows[1].field("Blocked by").unwrap().links,
        [(rows[0].id.clone(), scene::FIRST_ROW.to_owned())]
    );
    let first = tracker.show(&rows[0].id).await.unwrap();
    assert_eq!(first.body.as_deref(), Some(scene::FIRST_PAGE));
    assert_eq!(
        first.field("Blocking").unwrap().links,
        [(rows[1].id.clone(), scene::SECOND_ROW.to_owned())]
    );
    server.stop().await;
}
