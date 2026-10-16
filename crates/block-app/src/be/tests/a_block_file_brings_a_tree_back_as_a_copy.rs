use super::*;

fn exported(block: Uuid) -> (String, Vec<u8>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    export(block, move |exported| {
        let _ = sender.send(exported);
    });
    receiver
        .recv_timeout(PATIENCE)
        .expect("the export answers")
        .expect("the export succeeds")
}

#[test]
fn a_block_file_brings_a_tree_back_as_a_copy() {
    let harness = Harness::start();
    harness.connect();
    let folder = Uuid::new_v4();
    let mut named = be_block::BlockMetadata::default();
    named.insert(&be_block::NAME, &"Trip".to_owned());
    create(
        folder,
        be_block::FolderContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        named,
        None,
    );
    let counter = Uuid::new_v4();
    create(
        counter,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Block(folder),
        be_block::BlockMetadata::default(),
        None,
    );
    open(counter, CounterContent::CONTENT_TYPE);
    add(counter, 5);
    wait_for_count(counter, 5);

    let (name, while_open) = exported(folder);
    assert_eq!(name, "Trip.block");
    close(counter);
    let (_, after_closing) = exported(counter);

    for bytes in [while_open, after_closing] {
        let (top, top_type) =
            import(&bytes, be_graph::BlockParent::Root).expect("the file imports");
        assert_ne!(top, folder, "an import is a copy with ids of its own");
        let copy = query(Query::Children(top))
            .into_iter()
            .find(|node| node.content_type == CounterContent::CONTENT_TYPE)
            .map(|node| node.id);
        let copy = match top_type {
            kind if kind == be_block::FolderContent::CONTENT_TYPE => {
                assert_eq!(
                    node(top).and_then(|node| node.metadata.name()).as_deref(),
                    Some("Trip"),
                    "the copy keeps the name"
                );
                copy.expect("the folder's counter comes with it")
            }
            _ => top,
        };
        assert_ne!(copy, counter);
        open(copy, CounterContent::CONTENT_TYPE);
        wait_for_count(copy, 5);
    }
}
