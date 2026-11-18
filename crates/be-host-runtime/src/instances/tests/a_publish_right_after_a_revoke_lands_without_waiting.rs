use super::*;

use be_block_content::{BlockContent, LiveEdit};
use be_type_example_counter::{Counter, CounterContent};

/// A revoke reseals the open block under a new key, and an edit published right after it
/// lands under that key: the flush that sends it returns once the server holds it.
#[test]
fn a_publish_right_after_a_revoke_lands_without_waiting() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (block, _) = saved_counter();
    pollster::block_on(be_host_stack::create_agent("Helper".to_owned()))
        .expect("the agent is made");
    let agent = be_host_stack::wait_until("the agent is a contact", |shared| {
        shared
            .contacts
            .iter()
            .find(|contact| contact.display_name == "Helper" && contact.public.is_some())
            .map(|contact| contact.account)
    });
    let made = be_host_stack::wait_until("the block is made", |shared| {
        shared.graph.get(block).map(|node| node.changed)
    });
    be_host_stack::set_access(block, agent, be_block_graph::Access::Edit, |_| {});
    be_host_stack::wait_until("the mirror heard the grant", |shared| {
        shared
            .graph
            .get(block)
            .is_some_and(|node| node.changed > made)
            .then_some(())
    });
    let _instances = placed_on(block, CounterContent::CONTENT_TYPE);
    let head = be_host_stack::node(block).and_then(|node| node.head);

    be_host_stack::set_access(block, agent, be_block_graph::Access::None, |_| {});
    be_host_stack::operate(block, CounterContent::encode_operation(&Counter::add(1)));
    // A flush that stalls fails here, at the wait's limit.
    be_host_stack::flush_and_send();
    be_host_stack::wait_until("the edit landed", |shared| {
        shared
            .graph
            .get(block)
            .filter(|node| node.head.is_some() && node.head != head)
            .map(|_| ())
    });
}
