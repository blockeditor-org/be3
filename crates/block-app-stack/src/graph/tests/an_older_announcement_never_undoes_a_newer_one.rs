use super::*;

#[test]
fn an_older_announcement_never_undoes_a_newer_one() {
    let (note, gone) = (Uuid::new_v4(), Uuid::new_v4());
    let mut graph = Graph::default();
    listed(&mut graph, vec![node(gone)], 10);

    graph.put(node(note), 12);
    graph.remove(note, 11);
    assert!(
        graph.get(note).is_some(),
        "a removal from before it was made"
    );

    graph.put(node(gone), 9);
    graph.remove(gone, 10);
    graph.put(node(gone), 10);
    assert!(
        graph.get(gone).is_some(),
        "an announcement as new as the last"
    );
    graph.remove(gone, 13);
    graph.put(node(gone), 12);
    assert!(
        graph.get(gone).is_none(),
        "a change from before it was removed"
    );

    let unknown = Uuid::new_v4();
    graph.put(node(unknown), 9);
    assert!(
        graph.get(unknown).is_none(),
        "a change from before the graph was loaded"
    );
    graph.relist_all();
    graph.listed(Listing::Children(BlockParent::Root), Vec::new(), 2);
    graph.put(node(unknown), 3);
    assert!(
        graph.get(unknown).is_some(),
        "a reload starts over, as a restarted server does"
    );
}
