use super::*;
use be_sdk_api::{AccessLevel, Failure, embed::host_services::Call as HostCall, ops::Outcome};
use be_type_example_counter::CounterContent;

/// An app names, as its new block's id, a block the server holds that this device has never
/// listed (made from another device, under a folder not opened here): the host makes it here, the server refuses the create, and the
/// opening the app asked for meanwhile is answered refused by the host, never sent, and never
/// left waiting.
#[test]
fn an_app_opening_a_block_whose_create_the_server_refused_is_denied() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until("knew the counter type", |_| {
        be_host_stack::is_known(CounterContent::CONTENT_TYPE).then_some(())
    });
    let taken = harness.block_out_of_sight(CounterContent::CONTENT_TYPE);
    assert!(
        be_host_stack::node(taken).is_none(),
        "this device lists the block"
    );
    let mut instances = placed_on(Uuid::new_v4(), CounterContent::CONTENT_TYPE);
    call(
        &mut instances,
        Call::CreateBlock {
            reply: reply(next_op()),
            block: taken.into_bytes(),
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
            block: taken.into_bytes(),
            account: be_block_graph::ANYONE_INSIDE.into_bytes(),
            access: AccessLevel::View,
        },
    });

    be_host_stack::flush_and_send();
    let answered = loop {
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
        be_host_stack::take_updates_or_wait("the opening's answer", || {
            be_host_base::host::letters_posted() > seen
        });
    };
    let Outcome::Ok(bytes) = answered else {
        panic!("the opening failed as a call: {answered:?}");
    };
    let answer: Result<(), Failure> = be_sdk_api::ops::decode(&bytes).expect("an answer decodes");
    assert_eq!(
        answer,
        Err(Failure::Denied),
        "the host did not refuse opening a block whose create the server refused"
    );
}
