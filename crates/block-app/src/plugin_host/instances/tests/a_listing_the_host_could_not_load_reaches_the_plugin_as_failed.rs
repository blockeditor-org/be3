use super::*;

use std::time::Duration;

use be_block::CounterContent;
use block_plugin_api::BlockQuery;

#[derive(Debug, PartialEq, Eq)]
enum Sent {
    Blocks,
    Failed(String),
}

fn sent(instances: &mut Instances) -> Vec<Sent> {
    next_screens(instances)
        .opened
        .into_iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Blocks { .. }) => Some(Sent::Blocks),
            Message::Editor(EditorMessage::BlocksFailed { error, .. }) => Some(Sent::Failed(error)),
            _ => None,
        })
        .collect()
}

fn loaded(query: crate::be::Query) {
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        shared.graph.ready(query).then_some(())
    })
    .expect("the listing never loaded");
}

#[test]
fn a_listing_the_host_could_not_load_reaches_the_plugin_as_failed() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let folder = Uuid::new_v4();
    crate::be::create(
        folder,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    let mut instances = placed_on(folder, CounterContent::CONTENT_TYPE);
    instances.editor_message(EditorMessage::WatchBlocks {
        instance: INSTANCE,
        queries: vec![BlockQuery::Children(folder.into_bytes())],
    });
    let children = crate::be::Query::Children(folder);
    sent(&mut instances);
    loaded(children);
    assert_eq!(sent(&mut instances), [Sent::Blocks]);

    crate::be::fail_listings(children, "Couldn't load what's inside this block");
    assert_eq!(
        sent(&mut instances),
        [Sent::Failed(
            "Couldn't load what's inside this block".to_owned()
        )],
        "a listing the host could not load is not an empty one"
    );
    assert!(sent(&mut instances).is_empty(), "a failure is sent once");

    crate::be::refetch(children);
    loaded(children);
    assert_eq!(
        sent(&mut instances),
        [Sent::Blocks],
        "once a retry loads it, the answer is sent again"
    );
}
