use super::*;
use be_block::ChildChange;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

/// A lease of no length lapses at once: the device journals on as owner until a `Lease` tells
/// it otherwise, and the server answers its rebases and appends as done without keeping them,
/// not with an error the person would see.
#[test]
fn an_owner_whose_lease_lapsed_journals_on_without_an_error() {
    let harness = Harness::lapsing();
    harness.connect();
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Team"),
        None,
        false,
    );
    wait_until("knew the folder's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    let note = Uuid::new_v4();
    create(
        note,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block::BlockMetadata::named("Note"),
        Some(TextBlock::of("").encode()),
        false,
    );
    assert_eq!(
        change_child(
            workspace,
            WorkspaceContent::CONTENT_TYPE,
            ChildChange::Add(note)
        ),
        Some(true)
    );
    open(note, TextContent::CONTENT_TYPE);
    type_text(note, 0, "socks");
    flush();
    wait_until("sent what was typed", |shared| {
        shared.connected && shared.waiting == 0 && text_of(shared, note).as_deref() == Some("socks")
    });
    type_text(note, 5, " and boots");
    flush();
    wait_until("sent what was typed next", |shared| {
        shared.connected
            && shared.waiting == 0
            && text_of(shared, note).as_deref() == Some("socks and boots")
    });
    assert_eq!(with_shared(|shared| shared.error.clone()).flatten(), None);
}
