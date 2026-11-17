use super::*;

/// One editor holding two tentative gestures (two fingers) ends each on its own: cancelling one
/// drops only its edits, the layer commits when the other ends, and leaving cancels every one
/// still open.
#[test]
fn one_editors_two_gestures_end_on_their_own() {
    let mut holds = Holds::default();
    gesture_in(&mut holds, 1, 7, Gesture::BeginTentative);
    let first = serial(&operate_in(&mut holds, 1, 7));
    let other = gesture_in(&mut holds, 1, 8, Gesture::BeginTentative);
    assert_eq!(other.opened, Some(named(1, 8)));
    assert!(other.ended.is_empty(), "a second gesture leaves the first open");
    let second = serial(&operate_in(&mut holds, 1, 8));
    let third = serial(&operate_in(&mut holds, 1, 7));
    assert_eq!(
        holds
            .message(
                1,
                Some(&FromEditor::Operate {
                    gesture: None,
                    operation: vec![1],
                }),
            )
            .this,
        This::Counted,
        "an edit outside both"
    );

    let cancel = gesture_in(&mut holds, 1, 8, Gesture::Cancel);
    assert_eq!(cancel.ended, [(named(1, 8), false)]);
    assert!(!cancel.settled);
    assert!(cancel.released.contains(&second));
    assert_eq!(holds.open(), [named(1, 7)]);

    let commit = gesture_in(&mut holds, 1, 7, Gesture::Commit);
    assert!(commit.settled);
    assert_eq!(commit.counted, [first, third]);

    gesture_in(&mut holds, 1, 7, Gesture::BeginTentative);
    gesture_in(&mut holds, 1, 8, Gesture::BeginTentative);
    let gone = holds.detach(1);
    assert_eq!(gone.ended, [(named(1, 7), false), (named(1, 8), false)]);
    assert!(gone.settled && !holds.holding());
}
