use be_type_database::database_view::DatabaseViewContent;

use super::*;
use crate::session::connect;

/// A database whose view only its owner reads is as private as the view: downloaded, by its
/// owner or by another member, it would otherwise take the view along (the view is exported
/// whole) into a file that opens for anyone, so neither writes a file.
#[tokio::test]
async fn a_database_in_a_view_only_its_owner_reads_is_not_downloaded() {
    let harness = Harness::start().await;
    let owner = harness.owner().await;
    let settings = Uuid::now_v7();
    owner
        .create_block(
            settings,
            be_block_graph::SETTINGS_TYPE,
            BlockParent::Root,
            &BlockMetadata::default(),
            false,
        )
        .await
        .unwrap();
    let view = Uuid::now_v7();
    owner
        .create_block(
            view,
            DatabaseViewContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            &BlockMetadata::named("Private"),
            true,
        )
        .await
        .unwrap();
    let database = Uuid::now_v7();
    owner
        .create_block(
            database,
            DatabaseContent::CONTENT_TYPE,
            BlockParent::Block(view),
            &BlockMetadata::named("Private"),
            false,
        )
        .await
        .unwrap();
    let tracker = Tracker::new(owner, sources());
    std::fs::create_dir_all(&harness.directory).unwrap();

    let file = harness.directory.join("private.block");
    let file = file.to_str().unwrap().to_owned();
    let refused = run(&tracker, &format!("{database} download {file}"))
        .await
        .unwrap_err();
    assert!(refused.contains("only you can read"), "{refused}");
    assert!(!std::path::Path::new(&file).exists());

    let (settings, _, _) = agent(&harness, &tracker, None).await;
    let member = Tracker::new(connect(&settings).await.unwrap(), sources());
    let file = harness.directory.join("member.block");
    let file = file.to_str().unwrap().to_owned();
    assert!(
        run(&member, &format!("{database} download {file}"))
            .await
            .is_err()
    );
    assert!(!std::path::Path::new(&file).exists());
    harness.stop().await;
}
