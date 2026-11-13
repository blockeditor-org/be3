use super::*;

#[test]
fn what_a_chat_adds_over_the_servers_is_its_unsent_messages() {
    let server = sent(&ChatContent::default(), ADA, &["lunch at noon?"]);
    // The device's kept commit holds Ada's reply, which it relayed and sealed, beside Bo's own.
    let local = sent(&sent(&server, BO, &["running late"]), ADA, &["no rush"]);
    let local = sent(&local, BO, &["ten minutes"]);
    let unsent = |account| -> Vec<String> {
        local
            .unsent_over(&server, account)
            .iter()
            .flat_map(ChatContent::sent_in)
            .filter_map(|id| local.body(id))
            .collect()
    };
    assert_eq!(
        unsent(BO),
        ["running late", "ten minutes"],
        "only this account's own messages are its to show as sending"
    );
    assert!(
        server.unsent_over(&server, BO).is_empty(),
        "nothing is unsent over itself"
    );
}
