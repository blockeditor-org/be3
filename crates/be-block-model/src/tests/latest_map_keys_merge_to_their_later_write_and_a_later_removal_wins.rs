use super::*;

#[test]
fn latest_map_keys_merge_to_their_later_write_and_a_later_removal_wins() {
    let mut base = Document::new(&View::default());
    base.apply(
        &[
            View::STATE.put(ObjectId::ROOT, &"a".to_owned(), Some(&1), stamp(1, 1)),
            View::STATE.put(ObjectId::ROOT, &"b".to_owned(), Some(&1), stamp(1, 1)),
        ]
        .into_iter()
        .collect(),
    );
    let mut ours = base.clone();
    ours.apply(
        &[
            View::STATE.put(ObjectId::ROOT, &"a".to_owned(), Some(&2), stamp(2, 1)),
            View::STATE.put(ObjectId::ROOT, &"b".to_owned(), Some(&2), stamp(2, 1)),
        ]
        .into_iter()
        .collect(),
    );
    let mut theirs = base.clone();
    theirs.apply(
        &[
            View::STATE.put(ObjectId::ROOT, &"a".to_owned(), None, stamp(3, 2)),
            View::STATE.put(ObjectId::ROOT, &"c".to_owned(), Some(&3), stamp(1, 2)),
        ]
        .into_iter()
        .collect(),
    );

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);
    let state = merged.root().state;

    assert_eq!(conflicts, 0);
    assert_eq!(state.get("a"), None);
    assert_eq!(state.get("b"), Some(&2));
    assert_eq!(state.get("c"), Some(&3));
    assert_eq!(state.stamp(&"a".to_owned()), stamp(3, 2));
}
