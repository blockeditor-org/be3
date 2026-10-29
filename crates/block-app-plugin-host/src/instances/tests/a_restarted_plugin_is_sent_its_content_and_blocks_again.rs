use super::*;

use std::time::Duration;

use block_plugin_api::BlockQuery;
use counter_types::CounterContent;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Sent {
    Content,
    Blocks,
}

fn sent_once(instances: &mut Instances, block: Uuid, watch: OpId) -> Vec<Sent> {
    next_screens(instances);
    let mut sent: Vec<Sent> = from_replica(instances, block)
        .into_iter()
        .map(|_| Sent::Content)
        .collect();
    let listed: Option<Result<Vec<block_plugin_api::BlockInfo>, String>> =
        answered(instances, watch);
    sent.extend(listed.map(|_| Sent::Blocks));
    sent
}

fn sent(instances: &mut Instances, block: Uuid, watch: OpId) -> Vec<Sent> {
    let mut sent = sent_once(instances, block, watch);
    block_app_stack::wait_for(Duration::from_secs(20), |shared| {
        (shared.blocks.contains_key(&block) && !shared.editor_frames.is_empty()).then_some(())
    });
    sent.extend(sent_once(instances, block, watch));
    sent.sort();
    sent
}

#[test]
fn a_restarted_plugin_is_sent_its_content_and_blocks_again() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    block_app_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
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
