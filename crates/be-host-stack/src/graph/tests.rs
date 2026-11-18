use super::*;

mod a_backref_counts_by_the_references_read_at_its_head;
mod a_block_made_here_stays_listed_until_an_answer_from_after_the_drain_sent_it;
mod a_block_placed_in_another_is_listed_as_its_reference_before_the_server_hears_of_it;
mod a_block_s_children_leave_out_what_its_metadata_points_to;
mod a_block_under_a_deleted_one_is_deleted_with_it;
mod a_blocks_references_are_listed_in_one_request;
mod a_blocks_references_are_read_from_its_head_not_its_summary;
mod a_blocks_references_listed_before_the_block_itself_are_kept;
mod a_child_added_at_a_head_not_read_yet_is_not_taken_for_deleted;
mod a_drained_head_moves_only_a_block_no_replica_reports_for;
mod a_folders_blocks_leave_once_nothing_lists_it;
mod a_head_an_open_block_reported_outlasts_what_is_listed_of_it;
mod a_head_change_copies_one_node_not_the_graph_a_seen_shares;
mod a_head_change_hands_over_one_node_not_the_graph;
mod a_listing_read_again_waits_on_the_server_not_on_what_shows_it;
mod a_listing_whose_request_got_no_answer_is_asked_for_again;
mod a_placed_child_its_parent_drops_again_is_noted;
mod a_root_listing_made_before_the_organization_was_known_lets_its_settings_go;
mod a_tree_is_followed_while_the_mirror_holds_what_lies_in_it;
mod an_older_announcement_never_undoes_a_newer_one;
mod what_is_asked_for_is_read_before_what_is_only_held;

fn node(id: Uuid) -> Node {
    Node {
        id,
        content_type: Uuid::nil(),
        author: None,
        parent: Some(BlockParent::Root),
        access: Access::Edit,
        references: None,
        metadata: BlockMetadata::default(),
        head: None,
        locked: false,
        held: true,
        sets_access: false,
        size: 0,
        tree_size: 0,
        changed: 0,
        entry: None,
        unread: 0,
    }
}

/// `node` as changed at `sequence`.
fn changed_at(node: Node, sequence: u64) -> Node {
    Node {
        changed: sequence,
        ..node
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
