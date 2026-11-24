use super::*;

use be_type_example_counter::CounterContent;

/// `be BLOCK host` (plans/headless_apps.md, step 2) holds the block's replica open, and once no
/// other device hosts it starts the type's host component, which claims the lease: this device
/// hosts the block, and the counter's component adds one to it.
#[test]
fn be_hosts_a_block_while_no_other_device_does() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until("knew the counter type", |_| {
        be_host_stack::is_known(CounterContent::CONTENT_TYPE).then_some(())
    });
    let block = Uuid::now_v7();
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    be_host_stack::flush_and_send();
    be_host_stack::wait_until("listed the counter", |shared| {
        shared.graph.get(block).map(|_| ())
    });

    let mut told = Vec::new();
    let mut hosted_and_added = false;
    crate::hosting::host_block(
        block,
        CounterContent::CONTENT_TYPE,
        Uuid::new_v4(),
        || {
            let hosted =
                be_host_stack::read_seen(|seen| seen.hosted.contains(&block)).unwrap_or(false);
            let added = be_host_stack::content(block)
                .and_then(|content| CounterContent::decode(content.bytes()).ok())
                .is_some_and(|counter| counter.root().value() >= 1);
            hosted_and_added = hosted && added;
            hosted_and_added
        },
        |said| told.push(said.to_owned()),
    )
    .expect("the host runs");
    assert!(
        hosted_and_added,
        "this device hosts the block and its component added one"
    );
    assert_eq!(
        told.first().map(String::as_str),
        Some("standing by until no one else hosts it")
    );
    assert!(
        told.iter().any(|said| said == "hosting"),
        "it said it hosts: {told:?}"
    );
}
