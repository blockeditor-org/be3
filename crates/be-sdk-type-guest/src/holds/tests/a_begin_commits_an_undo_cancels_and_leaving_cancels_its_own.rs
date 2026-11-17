use super::*;

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
    assert!(undo.undone && undo.counted.is_empty() && undo.ended.is_empty());
    assert_eq!(undo.voided, [gesture_of(2)]);
    assert_eq!(undo.released.len(), 4);
    // The voided gesture stays open: what its editor sends before it hears is held, and none
    // of it counts however it ends.
    let late = serial(&operate(&mut holds, 2));
    let commit = gesture(&mut holds, 2, Gesture::Commit);
    assert_eq!(commit.this, This::Ended);
    assert!(commit.settled && commit.counted.is_empty());
    assert!(commit.released.contains(&late));
    assert!(!holds.holding());

    gesture(&mut holds, 1, Gesture::BeginTentative);
    operate(&mut holds, 1);
    let gone = holds.detach(1);
    assert!(gone.settled && gone.counted.is_empty());
    assert_eq!(gone.released.len(), 2);
    assert_eq!(holds.detach(1), Took::default());
}
