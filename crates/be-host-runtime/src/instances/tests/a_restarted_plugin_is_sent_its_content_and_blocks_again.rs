use super::*;

use be_sdk_api::BlockQuery;
use be_type_example_counter::CounterContent;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Sent {
    Content,
    Blocks,
}

fn sent(instances: &mut Instances, block: Uuid, watch: OpId) -> Vec<Sent> {
    next_screens(instances);
    be_host_stack::wait_until(|shared| {
        (shared.blocks.contains_key(&block) && shared.busy().is_empty()).then_some(())
    });
    next_screens(instances);
    let mut sent: Vec<Sent> = from_replica(instances, block)
        .into_iter()
        .map(|_| Sent::Content)
        .collect();
    let listed: Option<Result<Vec<be_sdk_api::BlockInfo>, String>> = answered(instances, watch);
    sent.extend(listed.map(|_| Sent::Blocks));
    sent.sort();
    sent
}

#[test]
fn a_restarted_plugin_is_sent_its_content_and_blocks_again() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    let watching = |instances: &mut Instances| {
        open_watch(instances, |op| Call::BlocksWatchNew {
            resource: wire::BlocksWatch(op),
            query: BlockQuery::Block(block.into_bytes()),
        })
    };
    let listing = watching(&mut instances);
    to_replica(&mut instances, block, &FromEditor::Resend);
    assert_eq!(
        sent(&mut instances, block, listing),
        [Sent::Content, Sent::Blocks]
    );
    assert!(
        sent(&mut instances, block, listing).is_empty(),
        "nothing changed since"
    );

    instances.reopen();
    let listing = watching(&mut instances);
    to_replica(&mut instances, block, &FromEditor::Resend);

    assert_eq!(
        sent(&mut instances, block, listing),
        [Sent::Content, Sent::Blocks],
        "a restarted plugin starts with nothing, so it is sent everything again"
    );
}
