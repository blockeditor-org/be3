use super::*;

use be_block::{BlockContent, LiveEdit};
use block_plugin_api::blocks::BlockHistory;
use checklist_types::{Checklist, ChecklistContent};

fn histories(instances: &mut Instances, watch: OpId) -> Vec<BlockHistory> {
    next_screens(instances);
    answered(instances, watch).into_iter().collect()
}

#[test]
fn an_instance_watching_a_blocks_history_is_told_when_it_changes() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let mut instances = placed();
    next_screens(&mut instances);
    let watch = open_watch(&mut instances, |op| Call::HistoryWatchNew {
        resource: wire::HistoryWatch(op),
        block: block.into_bytes(),
    });
    let nothing = BlockHistory {
        head: None,
        can_undo: false,
        can_redo: false,
    };
    assert_eq!(histories(&mut instances, watch), [nothing]);
    assert!(histories(&mut instances, watch).is_empty());

    block_app_stack::open(block, ChecklistContent::CONTENT_TYPE);
    block_app_stack::operate(
        block,
        ChecklistContent::encode_operation(&Checklist::add("Standup").1),
    );
    block_app_stack::wait_until(|shared| {
        shared
            .histories
            .get(&block)
            .is_some_and(|history| history.can_undo)
            .then_some(())
    });

    assert_eq!(
        histories(&mut instances, watch),
        [BlockHistory {
            head: block_app_stack::node(block)
                .and_then(|node| node.head)
                .map(|head| *head.hash().as_bytes()),
            can_undo: true,
            ..nothing
        }]
    );
}
