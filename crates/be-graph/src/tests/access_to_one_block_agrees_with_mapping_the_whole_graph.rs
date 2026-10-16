use super::*;

#[test]
fn access_to_one_block_agrees_with_mapping_the_whole_graph() {
    varied_graphs(|graph| {
        for account in ACCOUNTS {
            let map = whole_graph_access_map(graph, account);
            for block in graph.ids() {
                assert_eq!(
                    graph.effective_access(block, account),
                    map[&block],
                    "access of {account} to {block}"
                );
                let inherited = graph.inherited(block, account);
                if inherited.can_view() || map[&block].can_view() {
                    assert_eq!(inherited, map[&block]);
                }
            }
            assert_eq!(graph.effective_access(id(0xdead), account), Access::None);
        }
    });
}
