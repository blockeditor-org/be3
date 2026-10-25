use super::*;

use std::time::Duration;

use be_block::{BlockContent, LiveEdit};
use block_plugin_api::StoredHead;
use counter_types::{Counter, CounterContent};

fn saved_head(block: Uuid, after: Option<be_commit::CommitId>) -> be_commit::CommitId {
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        shared
            .graph
            .get(block)
            .and_then(|node| node.head)
            .filter(|head| Some(*head) != after)
    })
    .expect("the block was never saved")
}

fn heads(instances: &mut Instances, watch: OpId) -> Vec<Result<Option<StoredHead>, String>> {
    next_screens(instances);
    crate::be::flush();
    instances.pending();
    answered(instances, watch).into_iter().collect()
}

fn stored(head: &Result<Option<StoredHead>, String>) -> (i64, [u8; 32]) {
    let head = head.as_ref().ok().and_then(Option::as_ref).expect("a head");
    assert_eq!(head.content_type, CounterContent::CONTENT_TYPE.into_bytes());
    let counter = CounterContent::decode(&head.bytes).expect("the stored bytes are a counter");
    (counter.root().value(), head.commit)
}

fn add(block: Uuid, by: i64) {
    crate::be::operate(block, CounterContent::encode_operation(&Counter::add(by)));
    crate::be::flush();
}

#[test]
fn an_instance_watching_a_blocks_head_is_sent_its_stored_bytes() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    crate::be::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    crate::be::open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    let first = saved_head(block, None);
    let mut instances = placed();
    next_screens(&mut instances);
    let watch = open_watch(&mut instances, |op| Call::HeadWatchNew {
        resource: wire::HeadWatch(op),
        block: block.into_bytes(),
    });

    let sent = heads(&mut instances, watch);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(stored(&sent[0]), (4, *first.hash().as_bytes()));
    assert!(
        heads(&mut instances, watch).is_empty(),
        "nothing was saved since"
    );

    add(block, 1);
    let second = saved_head(block, Some(first));

    let sent = heads(&mut instances, watch);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(stored(&sent[0]), (5, *second.hash().as_bytes()));
}
