use super::*;
use be_block::history::{EDIT_BURST, History};
use std::time::Instant;

fn titled(content: &CalendarContent) -> Vec<String> {
    content
        .root()
        .events
        .iter()
        .map(|event| event.title.clone())
        .collect()
}

fn recorded(
    content: &mut CalendarContent,
    history: &mut History<CalendarContent>,
    edit: Edit,
    origin: Option<u64>,
    at: Instant,
) {
    history.record(content, &edit, origin, Some(at));
    content.apply(&edit);
}

#[test]
fn a_gesture_is_one_undo_step_however_long_it_takes() {
    let mut content = CalendarContent::default();
    let mut history = History::default();
    let mine = Some(1);
    let started = Instant::now();
    let later = |bursts: u32| started + EDIT_BURST * 2 * bursts;

    history.begin(mine);
    let (event, add) = Calendar::add(&CalendarEvent::new("S", 540, 555));
    recorded(&mut content, &mut history, add, mine, later(0));
    for (burst, title) in ["St", "Sta", "Standup"].into_iter().enumerate() {
        let retitle = CalendarEvent::TITLE.set(event, &title.to_owned()).into();
        recorded(
            &mut content,
            &mut history,
            retitle,
            mine,
            later(burst as u32 + 1),
        );
    }
    let elsewhere = CalendarEvent::START.set(event, &600_i64).into();
    recorded(&mut content, &mut history, elsewhere, Some(2), later(4));
    history.commit(mine);
    assert_eq!(titled(&content), ["Standup"]);

    for operation in history.undo(&content) {
        content.apply(&operation);
    }
    assert!(
        titled(&content).is_empty(),
        "one undo takes back the whole gesture, spread over several bursts"
    );
    assert!(
        history.can_undo(),
        "the other origin's edit, made during the gesture, is a step of its own"
    );
}
