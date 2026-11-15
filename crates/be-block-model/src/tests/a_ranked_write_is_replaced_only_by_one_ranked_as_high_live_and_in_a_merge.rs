use super::*;

fn put(key: &str, rank: u64, value: Option<u32>, at: Stamp) -> Edit {
    [View::STATE.put(ObjectId::ROOT, &key.to_owned(), value.as_ref(), at.ranked(rank))]
        .into_iter()
        .collect()
}

/// A write given a rank keeps its key until one ranked as high or higher: a lower rank written
/// later, live or from the other side of a merge, changes nothing, the same rank goes to the
/// later write, a removal ranks like a value, and the rank is stored.
#[test]
fn a_ranked_write_is_replaced_only_by_one_ranked_as_high_live_and_in_a_merge() {
    let mut base = Document::new(&View::default());
    base.apply(&put("a", 1, Some(1), stamp(1, 1)));
    base.apply(&put("b", 1, Some(1), stamp(1, 1)));

    let mut live = base.clone();
    live.apply(&put("a", 3, Some(3), stamp(2, 1)));
    live.apply(&put("a", 2, Some(2), stamp(9, 2)));
    assert_eq!(live.root().state.get("a"), Some(&3), "a lower rank written later loses");
    assert_eq!(live.root().state.rank(&"a".to_owned()), 3);
    live.apply(&put("a", 3, Some(4), stamp(10, 2)));
    assert_eq!(live.root().state.get("a"), Some(&4), "the same rank goes to the later");
    live.apply(&put("a", 4, None, stamp(11, 2)));
    assert_eq!(live.root().state.get("a"), None, "a removal ranks like a value");
    // A plain write keeps the rank it was stamped after, so it still replaces.
    let next = live.root().state.next(&"a".to_owned(), 12, uuid::Uuid::from_u128(2));
    live.apply(
        &[View::STATE.put(ObjectId::ROOT, &"a".to_owned(), Some(&5), next)]
            .into_iter()
            .collect(),
    );
    assert_eq!(live.root().state.get("a"), Some(&5));
    assert_eq!(live.root().state.rank(&"a".to_owned()), 4);

    let mut ours = base.clone();
    ours.apply(&put("a", 4, Some(4), stamp(2, 1)));
    ours.apply(&put("b", 2, Some(2), stamp(2, 1)));
    let mut theirs = base.clone();
    theirs.apply(&put("a", 3, Some(3), stamp(9, 2)));
    theirs.apply(&put("b", 5, None, stamp(3, 2)));
    for (one, other) in [(&ours, &theirs), (&theirs, &ours)] {
        let (merged, conflicts) = Document::merge(&base, one, other);
        let state = merged.root().state;
        assert_eq!(conflicts, 0);
        assert_eq!(state.get("a"), Some(&4), "the lower rank stamped later loses");
        assert_eq!(state.get("b"), None);
        assert_eq!(state.rank(&"b".to_owned()), 5);

        let loaded = Document::<View>::from_bytes(&merged.to_bytes()).expect("it loads");
        assert_eq!(loaded.root().state, state, "the ranks are stored");
    }
}
