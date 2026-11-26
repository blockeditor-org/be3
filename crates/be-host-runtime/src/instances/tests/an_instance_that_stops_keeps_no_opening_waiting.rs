use super::*;
use be_sdk_api::{AccessLevel, embed::host_services::Call as HostCall};
use be_type_example_counter::CounterContent;

/// An app opens a block it made while the server is out of reach, so the opening waits
/// (`Services::openings`); the instance stops, and nothing is left waiting for it.
#[test]
fn an_instance_that_stops_keeps_no_opening_waiting() {
    let mut harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until("knew the counter type", |_| {
        be_host_stack::is_known(CounterContent::CONTENT_TYPE).then_some(())
    });
    let mut instances = placed_on(Uuid::new_v4(), CounterContent::CONTENT_TYPE);
    harness.stop_server();
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
    instances.serve(crate::wire::Service {
        instance: INSTANCE,
        call: HostCall::SetAccess {
            reply: reply(next_op()),
            block: made.into_bytes(),
            account: be_block_graph::ANYONE_INSIDE.into_bytes(),
            access: AccessLevel::View,
        },
    });
    assert_eq!(instances.services.openings.len(), 1, "the opening waits");

    instances.withdraw_all(INSTANCE);
    assert!(
        instances.services.openings.is_empty(),
        "a stopped instance's opening still waits"
    );
    harness.start_server();
}
