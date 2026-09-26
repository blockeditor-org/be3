use super::*;

#[test]
fn a_second_checkout_copies_the_tree_under_the_same_local_ids() {
    let harness = Harness::start();
    harness.connect();
    let versioned = start_versioning("hello\n");

    let second = second_checkout(&versioned);

    let (folder, note) = (
        copy_of(versioned.folder, second),
        copy_of(versioned.note, second),
    );
    let copied = node(folder).expect("the folder was copied");
    assert_eq!(copied.parent, be_graph::BlockParent::Block(second));
    assert_eq!(copied.metadata.local_id, Some(versioned.folder));
    assert_eq!(copied.references, vec![note]);
    assert_eq!(
        node(note).map(|node| node.parent),
        Some(be_graph::BlockParent::Block(folder))
    );

    hold(folder, be_block::FolderContent::CONTENT_TYPE);
    hold(note, be_block::TextContent::CONTENT_TYPE);
    wait_until("opened the copies", |shared| {
        text_of(shared, note).as_deref() == Some("hello\n") && shared.blocks.contains_key(&folder)
    });
    let listed = content(folder)
        .and_then(|held| be_block::FolderContent::decode(&held.bytes).ok())
        .map(|listing| listing.root().blocks());
    assert_eq!(
        listed,
        Some(vec![versioned.note]),
        "the copy's content names the note the way every checkout does"
    );

    with_graph(|graph| {
        let scope = graph.scope_of(note).expect("the copy is in a checkout");
        assert_eq!(graph.to_local(scope, note), versioned.note);
        assert_eq!(
            graph.to_real(
                scope,
                versioned.note,
                block_plugin_api::BlockIdRole::Existing
            ),
            note
        );
        let adopted = graph.scope_of(versioned.note).expect("the original is too");
        assert_eq!(graph.to_local(adopted, versioned.note), versioned.note);
        assert_eq!(
            graph.to_real(
                adopted,
                versioned.note,
                block_plugin_api::BlockIdRole::Existing
            ),
            versioned.note
        );
    });
}
