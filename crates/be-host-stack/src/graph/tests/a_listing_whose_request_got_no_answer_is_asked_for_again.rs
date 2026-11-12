use super::*;

#[test]
fn a_listing_whose_request_got_no_answer_is_asked_for_again() {
    let folder = Uuid::from_u128(1);
    let mut graph = Graph::default();
    graph.want(Owner::Host, vec![Query::Children(folder)]);
    let asked = graph.take_wanted();
    assert!(!asked.is_empty());
    assert!(graph.fetching(), "what was asked for is being fetched");

    // The connection closed under the request: nothing was answered, so nothing failed.
    for listing in &asked {
        graph.unfetch(*listing);
    }
    assert_eq!(graph.failed_listings(), 0);
    assert_eq!(graph.take_wanted().len(), asked.len(), "asked for again");

    // What the server did answer, with an error, is failed until something relists it.
    for listing in &asked {
        graph.failed(*listing, "no".to_owned());
        graph.unfetch(*listing);
    }
    assert_eq!(graph.failed_listings(), asked.len());
    assert!(graph.take_wanted().is_empty());
}
