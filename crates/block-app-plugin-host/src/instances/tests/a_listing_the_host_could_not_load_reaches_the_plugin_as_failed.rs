use super::*;

use block_plugin_api::BlockQuery;
use counter_types::CounterContent;

#[derive(Debug, PartialEq, Eq)]
enum Sent {
    Blocks,
    Failed(String),
}

fn sent(instances: &mut Instances, watch: OpId) -> Vec<Sent> {
    next_screens(instances);
    answered::<Result<Vec<block_plugin_api::BlockInfo>, String>>(instances, watch)
        .into_iter()
        .map(|answer| match answer {
            Ok(_) => Sent::Blocks,
            Err(error) => Sent::Failed(error),
        })
        .collect()
}

fn loaded(query: block_app_stack::Query) {
    block_app_stack::wait_until(|shared| {
        shared.graph.ready(query).then_some(())
    });
}

#[test]
fn a_listing_the_host_could_not_load_reaches_the_plugin_as_failed() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let folder = Uuid::new_v4();
    block_app_stack::create(
        folder,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    let mut instances = placed_on(folder, CounterContent::CONTENT_TYPE);
    let watch = open_watch(&mut instances, |op| Call::BlocksWatchNew {
        resource: wire::BlocksWatch(op),
        query: BlockQuery::Children(folder.into_bytes()),
    });
    let children = block_app_stack::Query::Children(folder);
    sent(&mut instances, watch);
    loaded(children);
    assert_eq!(sent(&mut instances, watch), [Sent::Blocks]);

    block_app_stack::fail_listings(children, "Couldn't load what's inside this block");
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

    block_app_stack::refetch(children);
    loaded(children);
    assert_eq!(
        sent(&mut instances, watch),
        [Sent::Blocks],
        "once a retry loads it, the answer is sent again"
    );
}
