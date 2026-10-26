use super::*;

use std::time::Duration;

use be_block::{BlockContent, LiveEdit};
use block_plugin_api::blocks::BlockHistory;
use calendar_types::{Calendar, CalendarContent, CalendarEvent};

fn histories(instances: &mut Instances, watch: OpId) -> Vec<BlockHistory> {
    next_screens(instances);
    answered(instances, watch).into_iter().collect()
}

#[test]
fn an_instance_watching_a_blocks_history_is_told_when_it_changes() {
    let harness = crate::be::Harness::start();
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

    crate::be::open(block, CalendarContent::CONTENT_TYPE);
    crate::be::operate(
        block,
        CalendarContent::encode_operation(
            &Calendar::add(&CalendarEvent::new("Standup", 540, 555)).1,
        ),
    );
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        shared
            .histories
            .get(&block)
            .is_some_and(|history| history.can_undo)
            .then_some(())
    })
    .expect("the calendar never had anything to undo");

    assert_eq!(
        histories(&mut instances, watch),
        [BlockHistory {
            head: crate::be::node(block)
                .and_then(|node| node.head)
                .map(|head| *head.hash().as_bytes()),
            can_undo: true,
            ..nothing
        }]
    );
}
