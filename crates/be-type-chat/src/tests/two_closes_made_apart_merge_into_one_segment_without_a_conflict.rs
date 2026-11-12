use std::collections::HashSet;

use super::*;

fn assert_whole(merged: &ChatContent, expected: &HashSet<ObjectId>) {
    let recent = merged.messages();
    let recent_set: HashSet<ObjectId> = recent.iter().copied().collect();
    assert_eq!(recent.len(), recent_set.len(), "no recent message twice");
    let closed: HashSet<ObjectId> = merged.closed_messages_held_as_bytes().into_iter().collect();
    assert!(
        recent_set.is_disjoint(&closed),
        "a message is recent or closed"
    );
    let seen: HashSet<ObjectId> = recent_set.union(&closed).copied().collect();
    assert_eq!(&seen, expected, "no message lost");
}

#[test]
fn two_closes_made_apart_merge_into_one_segment_without_a_conflict() {
    let base = long(120);
    let all: HashSet<ObjectId> = base.messages().into_iter().collect();
    let ours = closed(&base);
    let theirs = closed(&base);

    // The same oldest messages closed on both sides: one segment, each message once.
    for (a, b) in [(&ours, &theirs), (&theirs, &ours)] {
        let (merged, conflicts) = merged(&base, a, b);
        assert_eq!(conflicts, 0);
        assert_eq!(merged.segments(), ours.segments(), "one segment, not two");
        assert_whole(&merged, &all);
    }

    // A side that deleted a message first closed a different segment over the same messages,
    // and one that deleted the oldest closed from the next: neither merge conflicts or loses a
    // message, both segments stay (they overlap, and a reader shows each message once), and a
    // deleted one is not brought back to the recent messages.
    for gone in [3, 0] {
        let mut other = base.clone();
        let deleted = base.messages()[gone];
        other.apply(&base.delete(deleted, ADA, 9).into());
        let other = closed(&other);
        let mut expected = all.clone();
        expected.remove(&deleted);
        for (a, b) in [(&ours, &other), (&other, &ours)] {
            let (merged, conflicts) = merged(&base, a, b);
            assert_eq!(conflicts, 0, "a chat merge never conflicts");
            assert_eq!(merged.segments().len(), 2, "both segments stay");
            assert!(!merged.messages().contains(&deleted));
            let mut seen: HashSet<ObjectId> = merged.messages().into_iter().collect();
            seen.extend(merged.closed_messages_held_as_bytes());
            seen.remove(&deleted);
            assert_eq!(seen, expected, "no message lost");
            let recent = merged.messages();
            assert_eq!(
                recent.len(),
                recent.iter().copied().collect::<HashSet<_>>().len(),
                "no recent message twice"
            );
        }
    }

    // A third device's own close, merged after the first two.
    let gone = base.messages()[5];
    let mut third = base.clone();
    third.apply(&base.delete(gone, BO, 9).into());
    let third = closed(&third);
    let (first, conflicts) = merged(&base, &ours, &third);
    assert_eq!(conflicts, 0);
    let (second, conflicts) = merged(&base, &first, &theirs);
    assert_eq!(conflicts, 0);
    let mut seen: HashSet<ObjectId> = second.messages().into_iter().collect();
    seen.extend(second.closed_messages_held_as_bytes());
    seen.remove(&gone);
    let mut expected = all.clone();
    expected.remove(&gone);
    assert_eq!(seen, expected);
    let recent = second.messages();
    assert_eq!(
        recent.len(),
        recent.iter().copied().collect::<HashSet<_>>().len(),
        "no recent message twice"
    );
}
