use super::*;

mod a_block_under_a_deleted_one_is_deleted_with_it;
mod a_blocks_references_are_listed_in_one_request;
mod a_blocks_references_listed_before_the_block_itself_are_kept;
mod a_folders_blocks_leave_once_nothing_lists_it;
mod an_older_announcement_never_undoes_a_newer_one;

fn node(id: Uuid) -> Node {
    Node {
        id,
        content_type: Uuid::nil(),
        author: None,
        parent: Some(BlockParent::Root),
        access: Access::Edit,
        references: Vec::new(),
        metadata: BlockMetadata::default(),
        head: None,
        locked: false,
        held: true,
        sets_access: false,
    }
}

fn listed(graph: &mut Graph, nodes: Vec<Node>, revision: u64) {
    let mut wanted = vec![Query::Roots, Query::Detached];
    wanted.extend(nodes.iter().map(|node| Query::Block(node.id)));
    graph.want(Owner::Host, wanted);
    for parent in [BlockParent::Root, BlockParent::Detached] {
        let inside = nodes
            .iter()
            .filter(|node| node.parent == Some(parent))
            .cloned()
            .collect();
        graph.listed(Listing::Children(parent), inside, revision);
    }
    for node in nodes {
        graph.listed(Listing::Block(node.id), vec![node], revision);
    }
}
