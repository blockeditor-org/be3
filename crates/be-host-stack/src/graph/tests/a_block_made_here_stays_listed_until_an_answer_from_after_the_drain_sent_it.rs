use super::*;

#[test]
fn a_block_made_here_stays_listed_until_an_answer_from_after_the_drain_sent_it() {
    let session = Uuid::from_u128(1);
    let roots = Listing::Children(BlockParent::Root);
    let mut graph = Graph::default();
    graph.want(Owner::Host, vec![Query::Roots]);
    assert!(
        !graph.take_wanted().is_empty(),
        "the roots are being fetched"
    );

    // Made here, kept, and settled: the drain has not sent it to the server yet.
    graph.change(node(session));
    graph.made(session);
    assert!(graph.settle(session));
    graph.listed(roots, Vec::new(), 1);
    assert!(
        graph.get(session).is_some(),
        "the server cannot list a block the drain has not sent, so its answer leaves it listed"
    );

    // The roots are asked for while the drain sends it, and answered from before it arrived.
    graph.relist(roots);
    assert!(!graph.take_wanted().is_empty());
    graph.relist_around(session, Some(BlockParent::Root));
    graph.listed(roots, Vec::new(), 2);
    assert!(
        graph.get(session).is_some(),
        "an answer to a request from before the drain sent it leaves it listed"
    );

    assert!(
        graph.take_unlisted_held().is_empty(),
        "no answer has left out what this device holds yet"
    );

    // Asked for again after the drain sent it, the roots answer for it.
    assert!(
        !graph.take_wanted().is_empty(),
        "the roots are asked for again"
    );
    graph.listed(roots, Vec::new(), 3);
    assert!(
        graph.get(session).is_none(),
        "an answer from after it was sent that leaves it out is believed"
    );
    assert_eq!(
        graph.take_unlisted_held(),
        [UnlistedHeld {
            listing: roots,
            blocks: vec![session],
            sequence: Some(3),
            stale: false,
        }],
        "and the worker is told, to log what a list lost"
    );
}
