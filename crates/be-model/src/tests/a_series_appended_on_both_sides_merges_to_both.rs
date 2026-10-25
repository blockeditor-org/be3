use super::*;

#[test]
fn a_series_appended_on_both_sides_merges_to_both() {
    let base = trace(&[1.0]);
    let mut ours = base.clone();
    ours.apply(&Edit(vec![grown(&ours, &[1.0, 2.0])]));
    let mut theirs = base.clone();
    theirs.apply(&Edit(vec![grown(&theirs, &[1.0, 3.0])]));

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    assert_eq!(*merged.root().points, [1.0, 2.0, 3.0]);
}
