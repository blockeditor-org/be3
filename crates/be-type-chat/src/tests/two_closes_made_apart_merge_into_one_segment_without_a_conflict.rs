use std::collections::HashSet;

use super::*;

/// Every message of `merged` that shows (recent, or in a segment it holds as bytes), each once.
fn assert_whole(merged: &ChatContent, expected: &HashSet<ObjectId>, deleted: Option<ObjectId>) {
    let recent = merged.messages();
    let recent_set: HashSet<ObjectId> = recent.iter().copied().collect();
    assert_eq!(recent.len(), recent_set.len(), "no recent message twice");
    let closed = merged.closed_messages_held_as_bytes();
    let closed_set: HashSet<ObjectId> = closed.iter().copied().collect();
    assert_eq!(
        closed.len(),
        closed_set.len(),
        "no closed message twice: no two segments overlap"
    );
    assert!(
        recent_set.is_disjoint(&closed_set),
        "a message is recent or closed"
    );
    let mut seen: HashSet<ObjectId> = recent_set.union(&closed_set).copied().collect();
    if let Some(deleted) = deleted {
        assert!(
            !recent_set.contains(&deleted),
            "a deleted message is not brought back"
        );
        seen.remove(&deleted);
    }
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
        let (merged, conflicts) = merged_by_parts(&base, a, b, no_parts()).expect("merges");
        assert_eq!(conflicts, 0);
        assert_eq!(merged.segments(), ours.segments(), "one segment, not two");
        assert_whole(&merged, &all, None);
    }

    // A side that deleted a message first closed a different segment over the same messages, and
    // one that deleted the oldest closed from the next: ours stand, theirs go and give their
    // other messages back to the recent ones, and a deleted message is not brought back.
    for gone in [3, 0] {
        let mut other = base.clone();
        let deleted = base.messages()[gone];
        other.apply(&base.delete(deleted, ADA, 9).into());
        let other = closed(&other);
        let mut expected = all.clone();
        expected.remove(&deleted);
        for (a, b) in [(&ours, &other), (&other, &ours)] {
            let (merged, conflicts) = merged_by_parts(&base, a, b, no_parts()).expect("merges");
            assert_eq!(conflicts, 0, "a chat merge never conflicts");
            assert_eq!(merged.segments(), a.segments(), "ours stand");
            assert_whole(&merged, &expected, Some(deleted));
        }
    }

    // A third device's own close, merged after the first two.
    let gone = base.messages()[5];
    let mut third = base.clone();
    third.apply(&base.delete(gone, BO, 9).into());
    let third = closed(&third);
    let (first, _) = merged_by_parts(&base, &ours, &third, no_parts()).expect("merges");
    let (second, conflicts) = merged_by_parts(&base, &first, &theirs, no_parts()).expect("merges");
    assert_eq!(conflicts, 0);
    let mut expected = all.clone();
    expected.remove(&gone);
    assert_whole(&second, &expected, Some(gone));

    // A segment whose part no side has cannot be settled: the merge is not done without it.
    let view = round_trip(&ours);
    assert!(
        merged_by_parts(
            &base,
            &view,
            &closed(&{
                let mut other = base.clone();
                other.apply(&base.delete(base.messages()[3], ADA, 9).into());
                other
            }),
            no_parts()
        )
        .is_err()
    );

    // Without parts the merge keeps overlapping entries, each with its own part: nothing lost.
    let mut other = base.clone();
    other.apply(&base.delete(base.messages()[3], ADA, 9).into());
    let other = closed(&other);
    let (union, conflicts) = merged(&base, &ours, &other);
    assert_eq!(conflicts, 0);
    assert_eq!(union.segments().len(), 2);
}
