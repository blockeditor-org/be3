use super::*;

use be_sdk_api::{
    BlockQuery,
    blocks::{BlockSubject, BlockValue},
};
use be_type_example_counter::CounterContent;

#[derive(Debug, PartialEq, Eq)]
enum Sent {
    Blocks,
    Failed(String),
}

fn sent(instances: &mut Instances, watch: OpId) -> Vec<Sent> {
    next_screens(instances);
    answered::<BlockValue>(instances, watch)
        .into_iter()
        .map(|answer| match answer {
            BlockValue::Blocks(Ok(_)) => Sent::Blocks,
            BlockValue::Blocks(Err(error)) => Sent::Failed(error.to_string()),
            other => panic!("not a listing: {other:?}"),
        })
        .collect()
}

fn loaded(query: be_host_stack::Query) {
    be_host_stack::wait_until(|shared| shared.graph.ready(query).then_some(()));
}

#[test]
fn a_listing_the_host_could_not_load_reaches_the_app_as_failed() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let folder = Uuid::new_v4();
    be_host_stack::create(
        folder,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    let mut instances = placed_on(folder, CounterContent::CONTENT_TYPE);
    let watch = open_watch(&mut instances, |op| Call::BlockWatchNew {
        resource: wire::BlockWatch(op),
        subject: BlockSubject::Blocks(BlockQuery::Children(folder.into_bytes())),
    });
    let children = be_host_stack::Query::Children(folder);
    sent(&mut instances, watch);
    loaded(children);
    assert_eq!(sent(&mut instances, watch), [Sent::Blocks]);

    be_host_stack::fail_listings(children, "Couldn't load what's inside this block");
    assert_eq!(
        sent(&mut instances, watch),
        [Sent::Failed(
            "Couldn't load what's inside this block".to_owned()
        )],
        "a listing the host could not load is not an empty one"
    );
    assert!(
        sent(&mut instances, watch).is_empty(),
        "a failure is sent once"
    );

    be_host_stack::refetch(children);
    loaded(children);
    assert_eq!(
        sent(&mut instances, watch),
        [Sent::Blocks],
        "once a retry loads it, the answer is sent again"
    );
}
