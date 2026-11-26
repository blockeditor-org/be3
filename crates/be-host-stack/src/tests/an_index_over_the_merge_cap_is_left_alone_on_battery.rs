use super::*;
use be_block_content::ChildChange;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

/// Waits until search is idle and everything it wrote is sent.
fn settled() {
    crate::wait_until("settled search and sent what it wrote", |shared| {
        (shared.waiting == 0
            && shared
                .search
                .busy_with(crate::search::wants_index(shared))
                .is_none())
        .then_some(())
    });
}

#[test]
fn an_index_over_the_merge_cap_is_left_alone_on_battery() {
    let harness = Harness::start();
    harness.connect();
    // The person is away, but the device runs on its battery.
    crate::set_rest(crate::Rest {
        away: true,
        on_power: false,
    });
    // Every merge a batch's compaction could make is over this cap.
    crate::with_shared_mut(|shared| shared.search.merge_bytes = Some(1));
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Team"),
        None,
        false,
    );
    wait_until("knew the workspace's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    for word in ["apples", "bananas", "cherries"] {
        let block = Uuid::new_v4();
        create(
            block,
            TextContent::CONTENT_TYPE,
            be_block_graph::BlockParent::Block(workspace),
            be_block_content::BlockMetadata::named("Note"),
            Some(TextBlock::of("").encode()),
            false,
        );
        assert!(made(change_child(
            workspace,
            WorkspaceContent::CONTENT_TYPE,
            ChildChange::Add(block)
        )));
        open(block, TextContent::CONTENT_TYPE);
        type_text(block, 0, word);
        flush();
    }
    let peer = crate::with_shared(|shared| shared.peer.clone())
        .flatten()
        .expect("the host has a peer");
    crate::wait_until("indexed every note", |shared| {
        (!shared.search.idle_done(&peer)).then_some(())
    });
    settled();
    send(Command::Sync);
    flush();
    settled();
    assert_eq!(
        crate::with_shared(|shared| shared.search.idle_compactions),
        Some(0),
        "the index was compacted on battery"
    );

    // On mains power again, the idle pass merges what the cap left.
    crate::set_rest(crate::Rest::default());
    crate::wait_until("compacted the index on power", |shared| {
        (shared.search.idle_compactions > 0
            && shared.search.idle_done(&peer)
            && shared.waiting == 0)
            .then_some(())
    });
}
