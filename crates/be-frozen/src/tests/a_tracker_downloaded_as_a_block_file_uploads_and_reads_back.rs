use be_block::BlockContent;
use be_graph::BlockParent;
use workspace_ui_types::FolderContent;

use super::*;
use text_block_types::TextContent;

#[tokio::test]
async fn a_tracker_downloaded_as_a_block_file_uploads_and_reads_back() {
    let server = start(None).await;
    let tracker = owner(&server).await;
    let blocks = be_client::transfer::decode(TRACKER).unwrap();
    let uploaded = tracker.upload(blocks, BlockParent::Root).await.unwrap();
    assert_eq!(uploaded.blocks, 6);

    let notes = tracker.entry(uploaded.top).await.unwrap();
    assert_eq!(notes.name.as_deref(), Some(scene::NOTES));
    let folder = tracker
        .read::<FolderContent>(notes.id)
        .await
        .unwrap()
        .unwrap();
    let children = folder.root().blocks();
    assert_eq!(children.len(), 2);
    let mut texts = Vec::new();
    for child in &children {
        if tracker.entry(*child).await.unwrap().block_type == TextContent::CONTENT_TYPE {
            texts.push(*child);
        }
    }
    let [shopping] = texts.as_slice() else {
        panic!("one text block in the folder: {texts:?}");
    };
    let shopping = *shopping;
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
    let first = tracker.show(database, &rows[0].id).await.unwrap();
    assert_eq!(first.body.as_deref(), Some(scene::FIRST_PAGE));
    assert_eq!(
        first.field("Blocking").unwrap().links,
        [(rows[1].id.clone(), scene::SECOND_ROW.to_owned())]
    );
    server.stop().await;
}
