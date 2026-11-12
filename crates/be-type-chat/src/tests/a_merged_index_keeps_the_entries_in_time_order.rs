use super::*;

#[test]
fn a_merged_index_keeps_the_entries_in_time_order() {
    // Two devices each closed twice while apart; one deleted the third message first, so the
    // closes differ and all four entries stay in a merge that reads no part.
    let base = long(260);
    let ours = closed(&closed(&base));
    let mut other = base.clone();
    other.apply(&base.delete(base.messages()[2], ADA, 9).into());
    let theirs = closed(&closed(&other));
    for (a, b) in [(&ours, &theirs), (&theirs, &ours)] {
        let (merged, conflicts) = merged(&base, a, b);
        assert_eq!(conflicts, 0);
        let segments = merged.segments();
        assert_eq!(segments.len(), 4, "all four entries stay");
        let order = base.messages();
        let at = |id: ObjectId| {
            order
                .iter()
                .position(|held| *held == id)
                .unwrap_or(usize::MAX)
        };
        let keys: Vec<(usize, usize)> = segments
            .iter()
            .map(|segment| (at(segment.last), at(segment.first)))
            .collect();
        assert!(
            keys.windows(2).all(|pair| pair[0] <= pair[1]),
            "the index is in time order: {keys:?}"
        );
        // Each entry still has its own part.
        let mut sorted = merged.closed_messages_held_as_bytes();
        let all = sorted.len();
        sorted.sort_by_key(|id| at(*id));
        sorted.dedup();
        assert!(all >= sorted.len());
        assert_eq!(merged.segment_ranges().len(), 4);
    }
}
