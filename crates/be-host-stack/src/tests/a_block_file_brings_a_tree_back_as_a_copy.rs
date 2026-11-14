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
    let mut named = be_block_content::BlockMetadata::default();
    named.insert(&be_block_content::NAME, &"Trip".to_owned());
    create(
        folder,
        be_type_workspace::FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        named,
        None,
        false,
    );
    let inner = Uuid::new_v4();
    create(
        inner,
        be_type_workspace::FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(folder),
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    let counter = Uuid::new_v4();
    create(
        counter,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(folder),
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    open(counter, CounterContent::CONTENT_TYPE);
    add(counter, 5);
    wait_for_count(counter, 5);
    let deeper: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    for (count, block) in (1..).zip(&deeper) {
        create(
            *block,
            CounterContent::CONTENT_TYPE,
            be_block_graph::BlockParent::Block(inner),
            be_block_content::BlockMetadata::default(),
            None,
            false,
        );
        open(*block, CounterContent::CONTENT_TYPE);
        add(*block, count);
        wait_for_count(*block, count);
    }

    let (name, while_open) = exported(folder);
    assert_eq!(name, "Trip.block");
    close(counter);
    let (_, after_closing) = exported(counter);

    let mut copies = Vec::new();
    for bytes in [while_open, after_closing] {
        let (top, top_type) =
            import(&bytes, be_block_graph::BlockParent::Root, None).expect("the file imports");
        assert_ne!(top, folder, "an import is a copy with ids of its own");
        let copy = query(Query::Children(top))
            .into_iter()
            .find(|node| node.content_type == CounterContent::CONTENT_TYPE)
            .map(|node| node.id);
        let copy = match top_type {
            kind if kind == be_type_workspace::FolderContent::CONTENT_TYPE => {
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
        copies.push(copy);
        if top_type != be_type_workspace::FolderContent::CONTENT_TYPE {
            continue;
        }
        let inner_copy = query(Query::Children(top))
            .into_iter()
            .find(|node| node.content_type == be_type_workspace::FolderContent::CONTENT_TYPE)
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
    for copy in copies {
        add(copy, 10);
        wait_for_count(copy, 15);
    }
    open(counter, CounterContent::CONTENT_TYPE);
    assert_eq!(
        crate::wait_until(|shared| counted(shared, counter)),
        5,
        "editing a copy leaves the original as it was"
    );
}
