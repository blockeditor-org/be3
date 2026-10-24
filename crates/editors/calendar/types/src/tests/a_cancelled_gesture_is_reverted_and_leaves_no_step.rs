use super::*;
use be_block::history::History;
use std::time::Instant;

#[test]
fn a_cancelled_gesture_is_reverted_and_leaves_no_step() {
    let (mut content, event) = scheduled();
    let before = content.clone();
    let mut history = History::default();
    let mine = Some(1);

    history.begin(mine);
    for title in ["Stand", "Standing"] {
        let retitle: Edit = CalendarEvent::TITLE.set(event, &title.to_owned()).into();
        history.record(&content, &retitle, mine, Some(Instant::now()));
        content.apply(&retitle);
    }
    assert!(history.can_undo());

    for operation in history.cancel(&content, mine) {
        content.apply(&operation);
    }
    assert_eq!(content, before);
    assert!(!history.can_undo() && !history.can_redo());
    assert!(history.undo(&content).is_empty());
}
