use super::*;
use be_block_graph::BlockParent;
use be_type_text::TextContent;
use be_type_workspace::FolderContent;

/// No summary names what a block references, yet a file tree or picker shows a collapsed row
/// as expandable only when it holds something (`BlockInfo::references`): a folder shown inside
/// another, whose own children nothing asked for, has its content read all the same.
#[test]
fn a_collapsed_folder_inside_a_shown_one_knows_it_has_something_to_expand() {
    let harness = Harness::start();
    let other = Device::online(&harness);
    let peer = std::sync::Arc::clone(&other.peer.borrow());
    let (outer, inner, empty, note) = other.runtime.block_on(async {
        let outer = peer
            .create::<FolderContent>(BlockParent::Root)
            .await
            .expect("the other device makes a folder");
        let inner = peer
            .create::<FolderContent>(BlockParent::Block(outer))
            .await
            .expect("the other device makes a folder inside");
        let empty = peer
            .create::<FolderContent>(BlockParent::Block(outer))
            .await
            .expect("the other device makes an empty folder inside");
        let note = peer
            .create::<TextContent>(BlockParent::Block(inner))
            .await
            .expect("the other device makes a note in the inner folder");
        for (folder, children) in [(outer, vec![inner, empty]), (inner, vec![note])] {
            let mut content = FolderContent::default();
            for child in children {
                let add = content.root().add(child);
                content.apply(&add);
            }
            peer.save(folder, &content, None)
                .await
                .expect("the other device saves a folder");
        }
        peer.drain(|_| false)
            .await
            .expect("the other device sends it all");
        (outer, inner, empty, note)
    });

    harness.connect();
    ask_for(vec![Query::Children(outer)]);
    let expandable = |block| {
        with_shared(|shared| {
            shared
                .graph
                .get(block)
                .map(|node| crate::app_graph::info_of(node, false, None, false).references)
        })
        .flatten()
    };
    wait_until("read what the collapsed folders hold", |shared| {
        shared.graph.ready(Query::Children(outer))
            && [inner, empty].iter().all(|block| {
                shared
                    .graph
                    .get(*block)
                    .is_some_and(|node| node.references.is_some())
            })
    });
    assert_eq!(
        expandable(inner),
        Some(vec![note.into_bytes()]),
        "the collapsed folder lists what it holds, so its row expands"
    );
    assert_eq!(
        expandable(empty),
        Some(Vec::new()),
        "an empty one lists nothing"
    );
}
