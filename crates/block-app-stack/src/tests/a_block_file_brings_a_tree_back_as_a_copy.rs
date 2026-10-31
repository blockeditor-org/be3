use super::*;

fn exported(block: Uuid) -> (String, Vec<u8>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    export(block, move |exported| {
        let _ = sender.send(exported);
    });
    receiver
        .recv()
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
        workspace_ui_types::FolderContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        named,
        None,
    );
    let inner = Uuid::new_v4();
    create(
        inner,
        workspace_ui_types::FolderContent::CONTENT_TYPE,
        be_graph::BlockParent::Block(folder),
        be_block::BlockMetadata::default(),
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
    let deeper: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    for (count, block) in (1..).zip(&deeper) {
        create(
            *block,
            CounterContent::CONTENT_TYPE,
            be_graph::BlockParent::Block(inner),
            be_block::BlockMetadata::default(),
            None,
        );
        open(*block, CounterContent::CONTENT_TYPE);
        add(*block, count);
        wait_for_count(*block, count);
    }

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
            kind if kind == workspace_ui_types::FolderContent::CONTENT_TYPE => {
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
        if top_type != workspace_ui_types::FolderContent::CONTENT_TYPE {
            continue;
        }
        let inner_copy = query(Query::Children(top))
            .into_iter()
            .find(|node| node.content_type == workspace_ui_types::FolderContent::CONTENT_TYPE)
            .expect("the inner folder comes with it")
            .id;
        let mut counts: Vec<i64> = query(Query::Children(inner_copy))
            .into_iter()
            .map(|node| {
                open(node.id, CounterContent::CONTENT_TYPE);
                crate::wait_until(|shared| counted(shared, node.id).filter(|count| *count > 0))
            })
            .collect();
        counts.sort_unstable();
        assert_eq!(
            counts,
            vec![1, 2, 3],
            "every block a level down comes with it"
        );
    }
}
