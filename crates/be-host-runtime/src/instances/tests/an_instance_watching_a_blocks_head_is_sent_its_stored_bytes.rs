use super::*;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_api::{
    Failure, StoredHead,
    blocks::{BlockSubject, BlockValue},
};
use be_type_example_counter::{Counter, CounterContent};

fn saved_head(block: Uuid, after: Option<be_block_commit::CommitId>) -> be_block_commit::CommitId {
    be_host_stack::wait_until(|shared| {
        shared
            .graph
            .get(block)
            .and_then(|node| node.head)
            .filter(|head| Some(*head) != after)
    })
}

fn heads(instances: &mut Instances, watch: OpId) -> Vec<Result<Option<StoredHead>, Failure>> {
    next_screens(instances);
    be_host_stack::flush_and_send();
    instances.pending();
    answered::<BlockValue>(instances, watch)
        .into_iter()
        .map(|value| match value {
            BlockValue::Head(head) => head,
            other => panic!("not a head: {other:?}"),
        })
        .collect()
}

fn stored(head: &Result<Option<StoredHead>, Failure>) -> (i64, [u8; 32]) {
    let head = head.as_ref().ok().and_then(Option::as_ref).expect("a head");
    assert_eq!(head.content_type, CounterContent::CONTENT_TYPE.into_bytes());
    let counter = CounterContent::decode(&head.bytes).expect("the stored bytes are a counter");
    (counter.root().value(), head.commit)
}

fn add(block: Uuid, by: i64) {
    be_host_stack::operate(block, CounterContent::encode_operation(&Counter::add(by)));
    be_host_stack::flush_and_send();
}

#[test]
fn an_instance_watching_a_blocks_head_is_sent_its_stored_bytes() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    be_host_stack::open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    let first = saved_head(block, None);
    let mut instances = placed();
    next_screens(&mut instances);
    let watch = open_watch(&mut instances, |op| Call::BlockWatchNew {
        resource: wire::BlockWatch(op),
        subject: BlockSubject::Head(block.into_bytes()),
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
