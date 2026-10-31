use super::*;

#[test]
fn blocks_moved_renamed_and_deleted_offline_reach_the_server_on_reconnect() {
    let mut harness = Harness::start();
    harness.connect();
    let address = harness.address();
    let (folder, inside, note) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    ask_for(vec![
        Query::Roots,
        Query::Block(folder),
        Query::Block(inside),
        Query::Block(note),
    ]);
    create(
        folder,
        workspace_ui_types::FolderContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Folder"),
        None,
    );
    create(
        inside,
        text_block_types::TextContent::CONTENT_TYPE,
        be_graph::BlockParent::Block(folder),
        be_block::BlockMetadata::named("Inside"),
        Some(text_block_types::TextBlock::of("inside").encode()),
    );
    written(note, "a note");
    wait_until("listed the blocks", |shared| {
        [folder, inside, note]
            .iter()
            .all(|block| shared.graph.get(*block).is_some())
    });

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    set_parent(inside, be_graph::BlockParent::Root);
    set_name(note, Some("Renamed".to_owned()));
    set_parent(folder, be_graph::BlockParent::Detached);
    flush();
    assert!(status().waiting >= 3, "the offline changes were not queued");
    stop();

    harness.connect();
    ask_for(vec![
        Query::Block(folder),
        Query::Block(inside),
        Query::Block(note),
    ]);
    let seen = |shared: &Shared| {
        let parent = |block| shared.graph.get(block).and_then(|node| node.parent);
        let name = shared.graph.get(note).and_then(|node| node.metadata.name());
        (parent(inside), parent(folder), name)
    };
    let expected = (
        Some(be_graph::BlockParent::Root),
        Some(be_graph::BlockParent::Detached),
        Some("Renamed".to_owned()),
    );
    crate::wait_until(|shared| (seen(shared) == expected).then_some(()));

    harness.start_server(&address);
    retry_now();
    wait_until("reconnected and sent the queued changes", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    let (inside_parent, folder_parent, name) = runtime().block_on(async {
        let peer = harness.outside_peer().await;
        let inside = peer.summary(inside).await.expect("the moved block exists");
        let folder = peer
            .summary(folder)
            .await
            .expect("the deleted block exists");
        let note = peer.summary(note).await.expect("the renamed block exists");
        let name = peer
            .metadata_of(note.head.expect("the note has content"))
            .await
            .expect("the note's metadata opens")
            .name();
        (inside.parent, folder.parent, name)
    });
    assert_eq!(inside_parent, Some(be_graph::BlockParent::Root));
    assert_eq!(folder_parent, Some(be_graph::BlockParent::Detached));
    assert_eq!(name.as_deref(), Some("Renamed"));
}
