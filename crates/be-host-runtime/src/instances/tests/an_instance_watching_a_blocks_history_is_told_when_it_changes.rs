use super::*;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_api::blocks::{BlockHistory, BlockSubject, BlockValue};
use be_type_checklist::{Checklist, ChecklistContent};

fn histories(instances: &mut Instances, watch: OpId) -> Vec<BlockHistory> {
    next_screens(instances);
    answered::<BlockValue>(instances, watch)
        .into_iter()
        .map(|value| match value {
            BlockValue::History(history) => history,
            other => panic!("not a history: {other:?}"),
        })
        .collect()
}

#[test]
fn an_instance_watching_a_blocks_history_is_told_when_it_changes() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let mut instances = placed();
    next_screens(&mut instances);
    let watch = open_watch(&mut instances, |op| Call::BlockWatchNew {
        resource: wire::BlockWatch(op),
        subject: BlockSubject::History(block.into_bytes()),
    });
    let nothing = BlockHistory {
        head: None,
        can_undo: false,
        can_redo: false,
        ..Default::default()
    };
    assert_eq!(histories(&mut instances, watch), [nothing]);
    assert!(histories(&mut instances, watch).is_empty());

    be_host_stack::open(block, ChecklistContent::CONTENT_TYPE);
    be_host_stack::operate(
        block,
        ChecklistContent::encode_operation(&Checklist::add("Standup").1),
    );
    be_host_stack::wait_until(|shared| {
        shared
            .histories
            .get(&block)
            .is_some_and(|history| history.can_undo)
            .then_some(())
    });

    assert_eq!(
        histories(&mut instances, watch),
        [BlockHistory {
            head: be_host_stack::node(block)
                .and_then(|node| node.head)
                .map(|head| *head.hash().as_bytes()),
            can_undo: true,
            ..nothing
        }]
    );
}
