use super::*;

fn rank(key: &str, rank: u64, value: Option<u32>, at: Stamp) -> Edit {
    [View::RANKED.rank(ObjectId::ROOT, &key.to_owned(), rank, value.as_ref(), at)]
        .into_iter()
        .collect()
}

/// A ranked key keeps the value of its highest rank, then latest stamp: a lower rank written
/// later, live or from the other side of a merge, changes nothing, and a removal is a value
/// like any other.
#[test]
fn ranked_map_keys_keep_their_highest_rank_live_and_in_a_merge() {
    let mut base = Document::new(&View::default());
    base.apply(&rank("a", 1, Some(1), stamp(1, 1)));
    base.apply(&rank("b", 1, Some(1), stamp(1, 1)));

    // Live: a higher rank replaces, a lower one does not even with a later stamp, and the same
    // rank goes to the later stamp.
    let mut live = base.clone();
    live.apply(&rank("a", 3, Some(3), stamp(2, 1)));
    live.apply(&rank("a", 2, Some(2), stamp(9, 2)));
    assert_eq!(live.root().ranked.get("a"), Some(&3));
    assert_eq!(live.root().ranked.rank(&"a".to_owned()), 3);
    live.apply(&rank("a", 3, Some(4), stamp(10, 2)));
    assert_eq!(live.root().ranked.get("a"), Some(&4));
    live.apply(&rank("a", 4, None, stamp(11, 2)));
    assert_eq!(live.root().ranked.get("a"), None, "a removal ranks like a value");
    assert_eq!(live.root().ranked.rank(&"a".to_owned()), 4);

    // In a merge: each key keeps its highest, whichever side wrote it and whatever its stamp.
    let mut ours = base.clone();
    ours.apply(&rank("a", 4, Some(4), stamp(2, 1)));
    ours.apply(&rank("b", 2, Some(2), stamp(2, 1)));
    let mut theirs = base.clone();
    theirs.apply(&rank("a", 3, Some(3), stamp(9, 2)));
    theirs.apply(&rank("b", 5, None, stamp(3, 2)));
    theirs.apply(&rank("c", 1, Some(1), stamp(3, 2)));
    for (one, other) in [(&ours, &theirs), (&theirs, &ours)] {
        let (merged, conflicts) = Document::merge(&base, one, other);
        let ranked = merged.root().ranked;
        assert_eq!(conflicts, 0);
        assert_eq!(ranked.get("a"), Some(&4), "the lower rank written later loses");
        assert_eq!(ranked.get("b"), None);
        assert_eq!(ranked.rank(&"b".to_owned()), 5);
        assert_eq!(ranked.get("c"), Some(&1));

        // And it is stored as it is.
        let loaded = Document::<View>::from_bytes(&merged.to_bytes()).expect("it loads");
        assert_eq!(loaded.root().ranked, ranked);
    }
}
