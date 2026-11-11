use std::collections::HashSet;

use super::*;

/// Every message of `chat`, in segments and recent, each once.
fn everything(chat: &ChatContent) -> Vec<ObjectId> {
    let mut all = chat.closed_messages();
    all.extend(chat.messages());
    all
}

fn assert_whole(merged: &ChatContent, expected: &HashSet<ObjectId>) {
    let all = everything(merged);
    let seen: HashSet<ObjectId> = all.iter().copied().collect();
    assert_eq!(all.len(), seen.len(), "no message twice");
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
    // and one that deleted the oldest closed from the next: neither merge conflicts, loses or
    // repeats a message, and a deleted one is not brought back.
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
            let mut seen: HashSet<ObjectId> = everything(&merged).into_iter().collect();
            // Ours' segment may hold the message the other side deleted; it is not shown.
            seen.remove(&deleted);
            assert_eq!(seen, expected, "no message lost");
            assert!(!merged.messages().contains(&deleted));
            let all = everything(&merged);
            assert_eq!(
                all.len(),
                all.iter().copied().collect::<HashSet<_>>().len(),
                "no message twice"
            );
        }
    }

    // A third device's own close, merged after the first two.
    let mut third = base.clone();
    third.apply(&base.delete(base.messages()[5], BO, 9).into());
    let third = closed(&third);
    let (first, conflicts) = merged(&base, &ours, &third);
    assert_eq!(conflicts, 0);
    let (second, conflicts) = merged(&base, &first, &theirs);
    assert_eq!(conflicts, 0);
    let mut seen: HashSet<ObjectId> = everything(&second).into_iter().collect();
    seen.remove(&base.messages()[5]);
    let mut expected = all.clone();
    expected.remove(&base.messages()[5]);
    assert_eq!(seen, expected);
}
