use super::*;
use be_type_workspace::FolderContent;

use be_block_content::ChildChange;

/// A block made in a folder and its backrefs listed at once: the folder's edit that adds it is
/// not sealed yet, so the server's answer, from what was published, leaves the folder out. The
/// folder's open replica says it points at the block, and that is what is listed, with nothing
/// logged as a listing that left out what this device holds.
#[test]
fn a_block_just_made_in_a_folder_lists_the_folder_among_its_backrefs() {
    // On a clock nothing moves, the folder's edit is never sealed by a timer.
    let harness = Harness::start().on_manual_clock();
    harness.connect();
    let (folder, child) = (Uuid::new_v4(), Uuid::new_v4());
    create(
        folder,
        FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    create(
        child,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(folder),
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    wait_until("knew the folder's type", |_| {
        is_known(FolderContent::CONTENT_TYPE)
    });
    // Made on the server before the edit: an edit made while the folder's creation waits in the
    // outbox is sealed as the server takes it (`Replicas::created`), so the folder joins its
    // session owning nothing. The peer took each creation (`Graph::changing`) and sent it: an
    // outbox the creation has not reached yet holds no creation either.
    wait_until("the server made both", |shared| {
        let Some(peer) = &shared.peer else {
            return false;
        };
        [folder, child].into_iter().all(|block| {
            shared.graph.get(block).is_some()
                && !shared.graph.changing(block)
                && !peer.creating(block)
        })
    });
    assert!(made(change_child(
        folder,
        FolderContent::CONTENT_TYPE,
        ChildChange::Add(child)
    )));
    wait_until("the folder's replica points at the child", |shared| {
        shared
            .graph
            .get(folder)
            .is_some_and(|node| node.refers_to(child) == Some(true))
    });

    ask_for(vec![Query::Backrefs(child)]);
    wait_until("listed the child's backrefs", |shared| {
        shared.graph.ready(Query::Backrefs(child))
    });
    let (unsealed, unlisted) = crate::wait_until("read what the answer did", |shared| {
        Some((
            shared.unsealed(),
            shared
                .unlisted_logged
                .iter()
                .chain(shared.graph.unlisted_held())
                .cloned()
                .collect::<Vec<_>>(),
        ))
    });
    assert!(
        unsealed > 0,
        "the folder's edit adding the child still waits for a seal, so the server cannot list it"
    );
    assert!(
        query(Query::Backrefs(child))
            .iter()
            .any(|node| node.id == folder),
        "the folder that points at the child is among its backrefs"
    );
    assert_eq!(
        unlisted,
        [],
        "an answer from before the folder's edit was published is not a listing that lost it"
    );
}
