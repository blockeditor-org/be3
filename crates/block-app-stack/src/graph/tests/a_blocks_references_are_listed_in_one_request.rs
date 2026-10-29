use super::*;

#[test]
fn a_blocks_references_are_listed_in_one_request() {
    let view = Uuid::from_u128(1);
    let rows: Vec<Uuid> = (2..200).map(Uuid::from_u128).collect();
    let linking = |references: Vec<Uuid>| Node {
        references,
        ..node(view)
    };
    let mut graph = Graph::default();
    graph.want(Owner::Editor(1), vec![Query::References(view)]);
    let mut wanted = graph.take_wanted();
    wanted.sort_by_key(|listing| format!("{listing:?}"));
    assert_eq!(
        wanted,
        [Listing::Block(view), Listing::References(view)],
        "the references come in one listing, not one request each"
    );
    graph.listed(Listing::Block(view), vec![linking(rows.clone())], 1);
    assert!(
        graph.take_wanted().is_empty(),
        "learning the block's references asks for nothing more"
    );
    assert!(!graph.ready(Query::References(view)));
    graph.listed(
        Listing::References(view),
        rows.iter().map(|row| node(*row)).collect(),
        1,
    );
    assert!(graph.ready(Query::References(view)));
    assert_eq!(graph.query(Query::References(view)).len(), rows.len());

    let added = Uuid::from_u128(500);
    let mut more = rows.clone();
    more.push(added);
    graph.put(linking(more), 2);
    assert_eq!(
        graph.take_wanted(),
        [Listing::References(view)],
        "a new reference lists the references again"
    );
    graph.put(node(added), 3);
    assert!(
        graph.get(added).is_some(),
        "news of a block the referrer links to is kept"
    );
}
