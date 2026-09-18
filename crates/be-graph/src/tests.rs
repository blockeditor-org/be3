use super::*;

mod a_cycle_is_refused_when_reparenting;
mod a_detached_subtree_is_pending_deletion;
mod access_is_inherited_through_parents;
mod object_references_are_counted_not_traced;
mod references_do_not_keep_a_block_alive;

const TYPE: Uuid = Uuid::from_u128(0xb10c);
const AUTHOR: Uuid = Uuid::from_u128(0xa170);

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn graph(parents: &[(u128, BlockParent)]) -> BlockGraph {
    let mut graph = BlockGraph::new();
    for (block, parent) in parents {
        graph
            .insert(id(*block), BlockNode::new(TYPE, AUTHOR, *parent))
            .unwrap();
    }
    graph
}
