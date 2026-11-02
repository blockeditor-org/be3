use super::*;

#[test]
fn latest_values_merge_to_the_later_write_without_a_conflict() {
    let base = Document::new(&View::default());
    let mut ours = base.clone();
    ours.apply(&View::SCROLL.set(ObjectId::ROOT, &10, stamp(5, 1)).into());
    let mut theirs = base.clone();
    theirs.apply(&View::SCROLL.set(ObjectId::ROOT, &20, stamp(7, 2)).into());

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);
    let (swapped, swapped_conflicts) = Document::merge(&base, &theirs, &ours);

    assert_eq!(conflicts, 0);
    assert_eq!(swapped_conflicts, 0);
    assert_eq!(*merged.root().scroll, 20);
    assert_eq!(merged, swapped);
}
