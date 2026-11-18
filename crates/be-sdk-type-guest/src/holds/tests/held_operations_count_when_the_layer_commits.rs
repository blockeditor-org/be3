use super::*;

/// A tentative gesture's begin, commit and cancel never count; its operations count once the
/// last open tentative gesture ends, in the order they arrived; a cancelled one's never do.
#[test]
fn held_operations_count_when_the_layer_commits() {
    let mut holds = Holds::default();
    let begin = gesture(&mut holds, 1, Gesture::BeginTentative);
    assert_eq!(begin.opened, Some(named(1, 0)));
    let first = serial(&operate(&mut holds, 1));
    let other = serial(&gesture(&mut holds, 2, Gesture::BeginTentative));
    let second = serial(&operate(&mut holds, 2));
    assert_eq!(
        operate(&mut holds, 3).this,
        This::Counted,
        "another editor types"
    );
    let third = serial(&operate(&mut holds, 1));

    let commit = gesture(&mut holds, 1, Gesture::Commit);
    assert!(
        matches!(commit.this, This::Held(_)),
        "kept until the layer commits"
    );
    assert!(!commit.settled && commit.counted.is_empty());

    let cancel = gesture(&mut holds, 2, Gesture::Cancel);
    assert_eq!(cancel.this, This::Ended);
    assert!(cancel.settled);
    assert_eq!(cancel.counted, [first, third]);
    assert!(cancel.released.contains(&other) && cancel.released.contains(&second));
    assert!(!holds.holding());
    assert_eq!(gesture(&mut holds, 1, Gesture::Commit).this, This::Counted);
}
