use super::*;
use be_type_workspace::FolderContent;

use be_block_content::ChildChange;

/// A child added to a folder whose creation still waits in the outbox (a device's My files made
/// as it started, on a slow link) stays in it once the drain sends the folder and its replica
/// joins the session: the folder's references keep the child throughout.
#[test]
fn a_child_added_to_a_folder_whose_creation_waits_stays_in_it_once_it_joins() {
    let harness = Harness::relayed();
    harness.connect();
    wait_until("loaded the graph", |shared| {
        shared.graph.ready(super::graph::Query::Roots)
    });
    wait_until("knew the folder's type", |_| {
        is_known(FolderContent::CONTENT_TYPE)
    });
    let mut joins = harness
        .relay()
        .count(|method, _| method == be_block_protocol::__args::SESSIONS_SESSION_JOIN);
    let joined_before = *joins.borrow_and_update();
    harness.relay().hold_requests(true);
    let (folder, child) = (Uuid::new_v4(), Uuid::new_v4());
    ask_for(vec![Query::Roots, Query::Children(folder)]);
    create(
        folder,
        FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("My files"),
        None,
        false,
    );
    create(
        child,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(folder),
        be_block_content::BlockMetadata::named("Text"),
        None,
        false,
    );
    assert_eq!(
        change_child(folder, FolderContent::CONTENT_TYPE, ChildChange::Add(child)),
        Some(true)
    );
    wait_until("listed the child in the folder", |shared| {
        shared
            .graph
            .get(folder)
            .is_some_and(|node| node.references.contains(&child))
    });
    harness.relay().hold_requests(false);
    flush_and_send();
    runtime()
        .block_on(joins.wait_for(|joins| *joins > joined_before))
        .expect("the folder's replica joined its session once it was sent");
    flush_and_send();
    let references =
        crate::wait_until(|shared| shared.graph.get(folder).map(|node| node.references.clone()));
    assert!(
        references.contains(&child),
        "the folder's references keep the child: {references:?}"
    );
    assert!(
        query(Query::Children(folder))
            .iter()
            .any(|node| node.id == child),
        "the folder still holds the child"
    );
}
