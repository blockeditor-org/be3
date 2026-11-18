use super::*;

/// What the worker hands over for a head another device published is that one node, however
/// large the graph, and applying it to the graph the page had gives the worker's.
#[test]
fn a_head_change_hands_over_one_node_not_the_graph() {
    let mut graph = Graph::default();
    graph.want(Owner::Host, vec![Query::Roots]);
    let nodes: Vec<Node> = (1..=20_000).map(|id| node(Uuid::from_u128(id))).collect();
    graph.listed(Listing::Children(BlockParent::Root), nodes, 1);
    let handed = graph.clone();
    let whole = postcard::to_stdvec(&graph).expect("it encodes").len();

    let remote = Uuid::from_u128(19_000);
    graph.set_head(
        remote,
        Some(CommitId::from_hash(be_block_store::Hash::of(b"published"))),
    );
    let delta = Diff::diff(&handed, &graph).expect("the head changed");
    let sent = postcard::to_stdvec(&delta).expect("it encodes");
    assert!(
        sent.len() * 100 < whole,
        "{} bytes handed over for one head, of {whole} for the graph",
        sent.len()
    );

    let mut page = handed.clone();
    page.apply(postcard::from_bytes(&sent).expect("it decodes"));
    assert!(page == graph, "the page's graph is the worker's");
    assert_eq!(
        page.get(remote).and_then(|node| node.head),
        graph.get(remote).and_then(|node| node.head)
    );
    assert!(
        Diff::diff(&graph, &graph.clone()).is_none(),
        "nothing changed, nothing is sent"
    );
}
