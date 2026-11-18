use super::*;

/// A workspace made here (My files, on signing in) is in the mirror before the peer kept its
/// creation, which pins the root of its log that an index's parent record names: an index made
/// in between is refused as locked, and shows "Making a search index failed". So none is made
/// until the creation settles, and settling moves the graph on, which looks again.
#[test]
fn a_workspace_made_here_gets_its_index_once_the_peer_kept_its_creation() {
    let workspace = Uuid::new_v4();
    let mut shared = Shared::default();
    shared.graph.want(Owner::Host, vec![Query::Roots]);
    shared.graph.change(Node {
        id: workspace,
        content_type: be_block::WORKSPACE_TYPE,
        author: None,
        parent: Some(be_block_graph::BlockParent::Root),
        access: be_block_graph::Access::Manage,
        references: None,
        metadata: be_block_content::BlockMetadata::default(),
        head: None,
        locked: false,
        held: true,
        sets_access: false,
        size: 0,
        tree_size: 0,
        changed: 0,
        entry: None,
        unread: 0,
    });
    assert!(shared.graph.get(workspace).is_some());
    assert_eq!(
        crate::search::unindexed(&shared),
        Vec::<Uuid>::new(),
        "the peer has yet to keep the workspace's creation"
    );
    assert!(!crate::search::wants_index(&shared));

    let revision = shared.graph.revision;
    assert!(shared.graph.settle(workspace));
    assert!(
        shared.graph.revision > revision,
        "settling moves the graph on, so the indexer looks again"
    );
    assert_eq!(crate::search::unindexed(&shared), vec![workspace]);
    assert!(crate::search::wants_index(&shared));
}
