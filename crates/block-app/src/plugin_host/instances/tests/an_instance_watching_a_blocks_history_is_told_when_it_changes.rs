use super::*;

use std::time::Duration;

use be_block::{BlockContent, Calendar, CalendarContent, CalendarEvent, LiveEdit};
use block_plugin_api::HistoryState;

fn history_states(instances: &mut Instances) -> Vec<Vec<HistoryState>> {
    instances
        .next_screens(PASS)
        .opened
        .into_iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::HistoryStates { states, .. }) => Some(states),
            _ => None,
        })
        .collect()
}

#[test]
fn an_instance_watching_a_blocks_history_is_told_when_it_changes() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let mut instances = placed();
    instances.next_screens(PASS);
    assert!(instances.editor_message(EditorMessage::WatchHistory {
        instance: INSTANCE,
        blocks: vec![block.into_bytes()],
    }));
    let nothing = HistoryState {
        block_id: block.into_bytes(),
        can_undo: false,
        can_redo: false,
    };
    assert_eq!(history_states(&mut instances), [vec![nothing]]);
    assert!(history_states(&mut instances).is_empty());

    crate::be::open(block, CalendarContent::CONTENT_TYPE);
    crate::be::operate_from(
        block,
        0,
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
        history_states(&mut instances),
        [vec![HistoryState {
            can_undo: true,
            ..nothing
        }]]
    );
}
