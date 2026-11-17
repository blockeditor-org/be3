use super::*;
use crate::editor::{FromEditor, Gesture};

fn gesture(holds: &mut Holds, editor: EditorId, gesture: Gesture) -> Took {
    holds.message(editor, Some(&FromEditor::Gesture(gesture)))
}

fn operate(holds: &mut Holds, editor: EditorId) -> Took {
    holds.message(editor, Some(&FromEditor::Operate(vec![1])))
}

fn serial(took: &Took) -> u64 {
    match took.this {
        This::Held(serial) => serial,
        this => panic!("held, not {this:?}"),
    }
}

/// A tentative gesture's begin, commit and cancel never count; its operations count once the
/// last open tentative gesture ends, in the order they arrived; a cancelled one's never do.
#[test]
fn held_operations_count_when_the_layer_commits() {
    let mut holds = Holds::default();
    let begin = gesture(&mut holds, 1, Gesture::BeginTentative);
    assert_eq!(begin.opened, Some(gesture_of(1)));
    let first = serial(&operate(&mut holds, 1));
    let other = serial(&gesture(&mut holds, 2, Gesture::BeginTentative));
    let second = serial(&operate(&mut holds, 2));
    assert_eq!(operate(&mut holds, 3).this, This::Counted, "another editor types");
    let third = serial(&operate(&mut holds, 1));

    let commit = gesture(&mut holds, 1, Gesture::Commit);
    assert!(matches!(commit.this, This::Held(_)), "kept until the layer commits");
    assert!(!commit.settled && commit.counted.is_empty());

    let cancel = gesture(&mut holds, 2, Gesture::Cancel);
    assert_eq!(cancel.this, This::Ended);
    assert!(cancel.settled);
    assert_eq!(cancel.counted, [first, third]);
    assert!(cancel.released.contains(&other) && cancel.released.contains(&second));
    assert!(!holds.holding());
    assert_eq!(gesture(&mut holds, 1, Gesture::Commit).this, This::Counted);
}

/// Beginning again under an open gesture's name commits it first; an undo cancels the whole
/// layer, committed gestures waiting for the rest too; an editor leaving cancels its own.
#[test]
fn a_begin_commits_an_undo_cancels_and_leaving_cancels_its_own() {
    let mut holds = Holds::default();
    gesture(&mut holds, 1, Gesture::BeginTentative);
    let first = serial(&operate(&mut holds, 1));
    let again = gesture(&mut holds, 1, Gesture::BeginTentative);
    assert_eq!(again.ended, [(gesture_of(1), true)]);
    assert!(again.settled);
    assert_eq!(again.counted, [first]);
    assert!(matches!(again.this, This::Held(_)));

    let group = gesture(&mut holds, 1, Gesture::Begin);
    assert_eq!(group.this, This::Counted);
    assert_eq!(group.ended, [(gesture_of(1), true)]);

    gesture(&mut holds, 1, Gesture::BeginTentative);
    operate(&mut holds, 1);
    gesture(&mut holds, 2, Gesture::BeginTentative);
    gesture(&mut holds, 1, Gesture::Commit);
    let undo = holds.undo();
    assert!(undo.settled && undo.counted.is_empty());
    assert_eq!(undo.ended, [(gesture_of(2), false)]);
    assert_eq!(undo.released.len(), 4);

    gesture(&mut holds, 1, Gesture::BeginTentative);
    operate(&mut holds, 1);
    let gone = holds.detach(1);
    assert!(gone.settled && gone.counted.is_empty());
    assert_eq!(gone.released.len(), 2);
    assert_eq!(holds.detach(1), Took::default());
}
