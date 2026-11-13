use super::*;

#[test]
fn what_a_chat_adds_over_the_servers_is_its_unsent_messages() {
    let server = sent(&ChatContent::default(), ADA, &["lunch at noon?"]);
    let local = sent(&server, BO, &["running late", "ten minutes"]);
    let unsent: Vec<String> = local
        .unsent_over(&server)
        .iter()
        .flat_map(ChatContent::sent_in)
        .filter_map(|id| local.body(id))
        .collect();
    assert_eq!(unsent, ["running late", "ten minutes"]);
    assert!(
        server.unsent_over(&server).is_empty(),
        "nothing is unsent over itself"
    );
}
