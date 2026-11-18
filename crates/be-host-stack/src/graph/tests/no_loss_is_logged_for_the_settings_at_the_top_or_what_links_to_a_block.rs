use super::*;

#[test]
fn no_loss_is_logged_for_the_settings_at_the_top_or_what_links_to_a_block() {
    let (settings, target, view) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let roots = Listing::Children(BlockParent::Root);
    let backrefs = Listing::Backrefs(target);
    let mut graph = Graph::default();
    graph.want(
        Owner::Host,
        vec![Query::Roots, Query::Backrefs(target), Query::Block(view)],
    );
    assert_eq!(graph.take_wanted().len(), 3, "all are being fetched");

    // The organization's settings sit at the top, which the server never lists, held while
    // their creation is pending; and a view the server lists as linking to `target`.
    graph.change(Node {
        content_type: SETTINGS_TYPE,
        ..node(settings)
    });
    let linking = Node {
        parent: Some(BlockParent::Block(target)),
        references: Some(vec![target]),
        ..node(view)
    };
    graph.listed(roots, Vec::new(), 1);
    graph.listed(backrefs, vec![changed_at(linking, 1)], 1);
    assert!(graph.settle(settings));

    // Answered again without either: the view's link to `target` is in a commit this device
    // holds ahead of what the server answered from.
    graph.relist(roots);
    graph.relist(backrefs);
    assert_eq!(graph.take_wanted().len(), 2);
    graph.listed(roots, Vec::new(), 2);
    graph.listed(backrefs, Vec::new(), 2);
    assert_eq!(
        graph.take_unlisted_held(),
        [],
        "the server lists the settings in no answer for the top, and what links to a block \
         places no file: neither loss is told"
    );

    // A block the top did list, then left out, is still told.
    graph.change(node(target));
    graph.listed(roots, Vec::new(), 3);
    assert!(graph.settle(target));
    graph.relist(roots);
    assert!(!graph.take_wanted().is_empty());
    graph.listed(roots, Vec::new(), 4);
    assert_eq!(
        graph.take_unlisted_held(),
        [UnlistedHeld {
            listing: roots,
            blocks: vec![target],
            sequence: 4,
            stale: false,
        }],
    );
}
