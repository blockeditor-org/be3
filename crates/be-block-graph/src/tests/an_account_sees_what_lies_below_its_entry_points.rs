use super::*;

#[test]
fn an_account_sees_what_lies_below_its_entry_points() {
    varied_graphs(|graph| {
        for account in ACCOUNTS {
            let mut known = whole_graph_access_map(graph, account);
            known.retain(|_, level| level.can_know_exists());
            assert_eq!(graph.visible(account), known, "what {account} sees");
        }
    });
}
