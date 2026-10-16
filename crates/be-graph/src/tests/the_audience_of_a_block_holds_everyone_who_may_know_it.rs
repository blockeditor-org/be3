use super::*;

#[test]
fn the_audience_of_a_block_holds_everyone_who_may_know_it() {
    varied_graphs(|graph| {
        let maps: Vec<(Uuid, BTreeMap<Uuid, Access>)> = ACCOUNTS
            .into_iter()
            .map(|account| (account, whole_graph_access_map(graph, account)))
            .collect();
        for block in graph.ids() {
            let audience = graph.audience(block);
            for below in graph.subtree(block) {
                for (account, map) in &maps {
                    if map[&below].can_know_exists() {
                        assert!(
                            audience.contains(account),
                            "{account} knows {below} below {block} but is not in its audience"
                        );
                    }
                }
            }
        }
    });
}
