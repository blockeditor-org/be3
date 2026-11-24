use super::*;

/// An edit made while a revoke of its block waits on the server in the lane waits its turn
/// behind it, and a flush sent after the edit, which needs no block, runs and answers ahead of
/// it. Once the revoke ends and the edit runs, it is sealed as the flush was asked, not at the
/// next quiet spell, which a held clock never brings.
#[test]
fn a_flush_that_runs_ahead_of_an_edit_waiting_behind_a_revoke_seals_it_once_it_runs() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();
    runtime()
        .block_on(crate::create_agent("Helper".to_owned()))
        .expect("the agent is made");
    let agent = crate::wait_until("the agent is a contact", |shared| {
        shared
            .contacts
            .iter()
            .find(|contact| contact.display_name == "Helper" && contact.public.is_some())
            .map(|contact| contact.account)
    });
    set_access(block, agent, be_block_graph::Access::Edit, |_| {});
    flush_and_send();
    let before = crate::node(block).and_then(|node| node.head);

    harness.relay().hold_answers(true);
    set_access(block, agent, be_block_graph::Access::None, |_| {});
    add(block, 2);
    flush();
    harness.relay().hold_answers(false);

    wait_until("sealed the edit the flush ran ahead of", |shared| {
        shared
            .graph
            .get(block)
            .is_some_and(|node| node.head.is_some() && node.head != before)
            && counted(shared, block) == Some(3)
            && shared.unsealed() == 0
            && shared.owing == 0
    });
}
