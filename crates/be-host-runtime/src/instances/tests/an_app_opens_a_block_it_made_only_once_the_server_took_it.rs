use super::*;
use be_sdk_api::{
    AccessLevel, Failure,
    embed::host_services::Call as HostCall,
    ops::Outcome,
};
use be_type_example_counter::CounterContent;

/// An app whose manifest does not set `sharing` opens a block it just made to anyone inside,
/// as the editor framework does for a settings kind. The opening waits for the server to take
/// the create, and goes to the server only then (which answers it, here refusing a kind that
/// is no settings kind), so an id the app named that the server already held, unseen here,
/// could never be opened by it.
#[test]
fn an_app_opens_a_block_it_made_only_once_the_server_took_it() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until("knew the counter type", |_| {
        be_host_stack::is_known(CounterContent::CONTENT_TYPE).then_some(())
    });
    let mut instances = placed_on(Uuid::new_v4(), CounterContent::CONTENT_TYPE);
    let made = Uuid::now_v7();
    call(
        &mut instances,
        Call::CreateBlock {
            reply: reply(next_op()),
            block: made.into_bytes(),
            block_type: CounterContent::CONTENT_TYPE.into_bytes(),
            parent: be_sdk_api::BlockLocation::Root,
            defined: Vec::new(),
            content: None,
            owner_only: false,
        },
    );
    let op = next_op();
    instances.take_answers();
    instances.serve(crate::wire::Service {
        instance: INSTANCE,
        call: HostCall::SetAccess {
            reply: reply(op),
            block: made.into_bytes(),
            account: be_block_graph::ANYONE_INSIDE.into_bytes(),
            access: AccessLevel::View,
        },
    });
    assert!(
        be_host_stack::node(made).is_some_and(|node| node.changed == 0),
        "the server took the create before the opening was asked"
    );
    assert!(
        instances.take_answers().iter().all(|(answered, _)| *answered != op),
        "the opening was answered before the server took the create"
    );

    be_host_stack::flush_and_send();
    be_host_stack::wait_until("the server took the create", |shared| {
        shared
            .graph
            .get(made)
            .filter(|node| node.changed > 0)
            .map(|_| ())
    });
    let opened = loop {
        let seen = be_host_base::host::letters_posted();
        next_screens(&mut instances);
        instances.pending();
        if let Some((_, outcome)) = instances
            .take_answers()
            .into_iter()
            .find(|(answered, _)| *answered == op)
        {
            break outcome;
        }
        // The stack's reply reaches the runtime's mailbox as its updates are taken.
        be_host_stack::take_updates_or_wait("the opening's answer", || {
            be_host_base::host::letters_posted() > seen
        });
    };
    let Outcome::Ok(bytes) = opened else {
        panic!("the opening was refused here, not by the server: {opened:?}");
    };
    let answer: Result<(), Failure> = be_sdk_api::ops::decode(&bytes).expect("an answer decodes");
    assert!(
        matches!(&answer, Err(Failure::Failed(why)) if why.contains("settings")),
        "the server did not judge the opening: {answer:?}"
    );
    assert!(
        be_host_stack::node(made).is_some_and(|node| node.changed > 0),
        "the opening was made before the server took the create"
    );
}
