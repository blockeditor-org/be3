use super::*;

#[test]
fn two_closes_made_apart_merge_into_one_segment_or_a_conflict() {
    let base = long(120);
    let ours = closed(&base);
    let theirs = closed(&base);

    // The same oldest messages closed on both sides: one segment, each message once.
    for (a, b) in [(&ours, &theirs), (&theirs, &ours)] {
        let (merged, conflicts) = merged(&base, a, b);
        assert_eq!(conflicts, 0);
        assert_eq!(merged.segments(), ours.segments(), "one segment, not two");
        assert_eq!(merged.messages(), ours.messages());
    }

    // A side that deleted a message first closed a different segment over the same messages.
    let mut other = base.clone();
    let gone = base.messages()[3];
    other.apply(&base.delete(gone, ADA, 9).into());
    let other = closed(&other);
    assert_ne!(other.segments(), ours.segments());
    for (a, b) in [(&ours, &other), (&other, &ours)] {
        let (_, conflicts) = merged(&base, a, b);
        assert_eq!(conflicts, 1, "overlapping segments are a conflict");
    }

    // Closes that open at different messages (one side deleted the oldest first) overlap too.
    let mut first_gone = base.clone();
    first_gone.apply(&base.delete(base.messages()[0], ADA, 9).into());
    let first_gone = closed(&first_gone);
    for (a, b) in [(&ours, &first_gone), (&first_gone, &ours)] {
        let (_, conflicts) = merged(&base, a, b);
        assert_eq!(conflicts, 1, "segments that overlap are a conflict");
    }
}
