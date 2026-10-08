use super::*;

use std::time::Duration;

use be_block::CounterContent;
use block_plugin_api::BlockQuery;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Sent {
    Content,
    Blocks,
}

fn sent_once(instances: &mut Instances) -> Vec<Sent> {
    next_screens(instances)
        .opened
        .into_iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Content { .. }) => Some(Sent::Content),
            Message::Editor(EditorMessage::Blocks { .. }) => Some(Sent::Blocks),
            _ => None,
        })
        .collect()
}

fn sent(instances: &mut Instances, block: Uuid) -> Vec<Sent> {
    let mut sent = sent_once(instances);
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        shared.blocks.contains_key(&block).then_some(())
    })
    .expect("the block's content never loaded");
    sent.extend(sent_once(instances));
    sent.sort();
    sent
}

#[test]
fn a_restarted_plugin_is_sent_its_content_and_blocks_again() {
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
    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    let watch = || EditorMessage::WatchBlocks {
        instance: INSTANCE,
        queries: vec![BlockQuery::Block(block.into_bytes())],
    };
    instances.editor_message(watch());
    assert_eq!(sent(&mut instances, block), [Sent::Content, Sent::Blocks]);
    assert!(
        sent(&mut instances, block).is_empty(),
        "nothing changed since"
    );

    instances.reopen();
    instances.editor_message(watch());

    assert_eq!(
        sent(&mut instances, block),
        [Sent::Content, Sent::Blocks],
        "a restarted plugin starts with nothing, so it is sent everything again"
    );
}
