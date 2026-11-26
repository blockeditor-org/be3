use super::*;

fn commit(name: &[u8]) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(name))
}

/// Every held block's references are read, but what is asked for (a folder just opened) goes
/// before what is only held, and one take is bounded, so it never waits behind a long tail.
#[test]
fn what_is_asked_for_is_read_before_what_is_only_held() {
    let opened = Uuid::from_u128(u128::MAX);
    let held: Vec<Uuid> = (1..=200).map(Uuid::from_u128).collect();
    let at = |id: Uuid| Node {
        head: Some(commit(id.as_bytes())),
        ..node(id)
    };
    let mut graph = Graph::default();
    graph.want(Owner::Host, vec![Query::Roots]);
    graph.take_wanted();
    graph.listed(
        Listing::Children(BlockParent::Root),
        held.iter().copied().map(at).collect(),
        1,
    );
    graph.want(
        Owner::Editor(1),
        vec![Query::Roots, Query::Children(opened)],
    );
    graph.take_wanted();
    graph.listed(Listing::Block(opened), vec![at(opened)], 1);

    let first = graph.take_to_read();
    assert_eq!(first[0].0, opened, "the folder opened is read first");
    assert!(first.len() < held.len(), "one take is bounded");
    let mut taken = first.len();
    while graph.has_to_read() {
        taken += graph.take_to_read().len();
    }
    assert_eq!(taken, held.len() + 1, "every held block is read in the end");
}
