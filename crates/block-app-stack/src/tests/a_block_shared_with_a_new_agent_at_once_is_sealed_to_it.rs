use super::*;

#[test]
fn a_block_shared_with_a_new_agent_at_once_is_sealed_to_it() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    written(block, "for the agent");
    wait_until("made the block", |shared| shared.graph.get(block).is_some());

    let runtime = runtime();
    runtime
        .block_on(create_agent("Helper".to_owned()))
        .expect("the agent is made");
    let agent = with_shared(|shared| {
        shared
            .contacts
            .iter()
            .find(|contact| contact.display_name == "Helper" && contact.public.is_some())
            .map(|contact| contact.account)
    })
    .flatten()
    .expect("the new agent is a contact with its key as soon as it is made");

    set_access(block, agent, be_graph::Access::Edit);
    flush();
    let (reply, listed) = std::sync::mpsc::channel();
    list_access(block, move |entries| {
        let _ = reply.send(entries);
    });
    let entries = listed
        .recv()
        .expect("the access is listed")
        .expect("the access list loads");
    let entry = entries
        .entries
        .iter()
        .find(|entry| entry.account == agent)
        .expect("the agent has access");
    assert!(
        !entry.waiting,
        "the block's key was sealed to the agent rather than left waiting"
    );
}
