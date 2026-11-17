use super::*;

/// What `settle` and `synced` hear of `graph`: its busy and its unsynced reasons.
fn reasons(graph: &Graph) -> (Vec<String>, Vec<String>) {
    let seen = crate::seen::Seen {
        graph: crate::Part::new(graph.clone()),
        connected: true,
        ..Default::default()
    };
    (seen.busy_with(false, 0, None), seen.unsynced())
}

/// A listing's first read is what shows it waits for (`loading`, busy); reading it again after
/// the server answered keeps showing what it held, so it waits on the server alone
/// (`refreshing`, unsynced), however the read again came about.
#[test]
fn a_listing_read_again_waits_on_the_server_not_on_what_shows_it() {
    let block = Uuid::from_u128(1);
    let listing = Listing::Backrefs(block);
    let mut graph = Graph::default();
    graph.want(Owner::Host, vec![Query::Backrefs(block)]);
    assert!(graph.loading(), "never answered: what shows it waits");
    assert!(!graph.refreshing());
    let (busy, unsynced) = reasons(&graph);
    assert_eq!(busy, ["the block store loads the organization's graph"]);
    assert!(unsynced.is_empty(), "{unsynced:?}");

    assert_eq!(graph.take_wanted(), vec![listing]);
    assert!(graph.loading(), "under way, still unanswered");
    graph.listed(listing, Vec::new(), 1);
    assert!(!graph.loading());
    assert!(!graph.refreshing());
    assert_eq!(reasons(&graph), (Vec::new(), Vec::new()));

    // A change the server pushed asks for it again.
    graph.relist(listing);
    assert!(!graph.loading(), "what it held keeps showing");
    assert!(graph.refreshing(), "it waits on the server");
    let (busy, unsynced) = reasons(&graph);
    assert!(busy.is_empty(), "{busy:?}");
    assert_eq!(unsynced, ["the block store reads parts of the graph again"]);

    // Under way, its request gets no answer (the connection closed under it): asked again.
    assert_eq!(graph.take_wanted(), vec![listing]);
    assert!(graph.refreshing(), "under way");
    graph.unfetch(listing);
    assert!(!graph.loading());
    assert!(graph.refreshing(), "asked for again");

    // The server answers with an error, and something lists it again.
    assert_eq!(graph.take_wanted(), vec![listing]);
    graph.failed(listing, "no".to_owned());
    assert!(!graph.refreshing(), "failed, nothing is under way");
    graph.relist(listing);
    assert!(!graph.loading());
    assert!(graph.refreshing(), "read again after it failed");

    assert_eq!(graph.take_wanted(), vec![listing]);
    graph.listed(listing, Vec::new(), 2);
    assert!(!graph.refreshing());
    assert_eq!(reasons(&graph), (Vec::new(), Vec::new()));
}
