use super::*;

use be_type_example_counter::CounterContent;

#[test]
fn a_block_an_app_makes_names_the_app_and_the_app_cannot_say_otherwise() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until(|_| {
        be_host_stack::is_known(CounterContent::CONTENT_TYPE).then_some(())
    });
    let own = Uuid::new_v4();
    let made = Uuid::now_v7();
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    instances.set_app_id("example.maker".to_owned());

    let mut claimed = be_block_content::BlockMetadata::default();
    claimed.insert(&be_block_content::CREATED_BY, &"someone.else".to_owned());
    call(
        &mut instances,
        Call::CreateBlock {
            reply: reply(next_op()),
            block: made.into_bytes(),
            block_type: CounterContent::CONTENT_TYPE.into_bytes(),
            parent: be_sdk_api::BlockLocation::Root,
            defined: be_host_stack::app_graph::wire_entries(&claimed.defined),
            content: None,
            owner_only: false,
        },
    );
    be_host_stack::flush_and_send();

    let global = be_host_stack::wait_until(|shared| shared.graph.get(made).map(|_| made));
    let created_by =
        || be_host_stack::node(global).and_then(|node| node.metadata.get(&be_block_content::CREATED_BY));
    assert_eq!(
        created_by().as_deref(),
        Some("example.maker"),
        "the host names the app that made the block, whatever the app sent"
    );
    call(
        &mut instances,
        Call::Act {
            reply: reply(next_op()),
            block: made.into_bytes(),
            command: be_sdk_api::BlockCommand::Define {
                key: be_block_content::CREATED_BY.id.into_bytes(),
                value: claimed.defined.get(&be_block_content::CREATED_BY.id).cloned(),
            },
        },
    );
    be_host_stack::flush_and_send();
    assert_eq!(
        created_by().as_deref(),
        Some("example.maker"),
        "nor can it rename the maker later"
    );
}
