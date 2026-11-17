use super::*;

/// A listing's first read is what shows it waits for (`loading`); reading it again after the
/// server answered keeps showing what it held, so it waits on the server alone (`refreshing`),
/// and a connection that silently stops answering never keeps the host busy.
#[test]
fn a_listing_read_again_waits_on_the_server_not_on_what_shows_it() {
    let block = Uuid::from_u128(1);
    let mut graph = Graph::default();
    graph.want(Owner::Host, vec![Query::Backrefs(block)]);
    assert!(graph.loading(), "never answered: what shows it waits");
    assert!(!graph.refreshing());

    let asked = graph.take_wanted();
    assert_eq!(asked, vec![Listing::Backrefs(block)]);
    assert!(graph.loading(), "under way, still unanswered");
    graph.listed(Listing::Backrefs(block), Vec::new(), 1);
    assert!(!graph.loading());
    assert!(!graph.refreshing());

    // A change the server pushed asks for it again.
    graph.relist(Listing::Backrefs(block));
    assert!(!graph.loading(), "what it held keeps showing");
    assert!(graph.refreshing(), "it waits on the server");
    graph.take_wanted();
    assert!(graph.refreshing(), "under way");
    graph.listed(Listing::Backrefs(block), Vec::new(), 2);
    assert!(!graph.refreshing());
}
