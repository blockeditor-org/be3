use super::*;

use be_block_content::{BlockMetadata, CREATED_BY};
use be_block_graph::BlockParent;
use be_sdk_api::{
    embed::host_services::Call,
    ops::{OpError, Outcome},
};
use be_type_example_counter::CounterContent;

fn made(block: Uuid, parent: BlockParent, by: &str) {
    let mut metadata = BlockMetadata::default();
    metadata.insert(&CREATED_BY, &by.to_owned());
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        parent,
        metadata,
        None,
        false,
    );
}

#[test]
fn an_app_makes_an_agent_for_a_block_it_made_only_while_it_sits_alone_in_unfiled() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let unfiled = BlockParent::Block(be_host_stack::unfiled().expect("signed in"));
    let own = Uuid::new_v4();
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    instances.set_app_id("be3.boat".to_owned());
    instances.allow_block_agents(true);
    let denied = vec![(1, Outcome::Err(OpError::Denied))];
    let mut ask = |block: Uuid| {
        instances.take_answers();
        instances.serve(crate::wire::Service {
            instance: INSTANCE,
            call: Call::CreateBlockAgent {
                reply: reply(1),
                block: block.into_bytes(),
                name: "boat VM".into(),
            },
        });
        instances.take_answers() == denied
    };

    let terminal = Uuid::now_v7();
    made(terminal, unfiled, "be3.boat");
    assert!(
        !ask(terminal),
        "a block the app made in Unfiled gets an agent"
    );

    let folder = Uuid::now_v7();
    made(folder, BlockParent::Root, "be3.files");
    be_host_stack::set_parent(terminal, BlockParent::Block(folder));
    assert!(
        ask(terminal),
        "the same block moved out of Unfiled does not"
    );

    let tagged = Uuid::now_v7();
    made(tagged, BlockParent::Block(folder), "be3.boat");
    assert!(
        ask(tagged),
        "a block outside Unfiled whose CREATED_BY names the app does not"
    );

    let filed_into = Uuid::now_v7();
    made(filed_into, unfiled, "be3.boat");
    made(Uuid::now_v7(), BlockParent::Block(filed_into), "be3.files");
    assert!(ask(filed_into), "nor one something was filed into");

    let anothers = Uuid::now_v7();
    made(anothers, unfiled, "be3.other");
    assert!(ask(anothers), "nor one another app made");
}
