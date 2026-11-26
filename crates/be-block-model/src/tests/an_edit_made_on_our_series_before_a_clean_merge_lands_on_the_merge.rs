use super::*;

/// As `an_edit_made_on_our_text_before_a_clean_merge_lands_on_the_merge`, for a series both
/// sides appended to.
#[test]
fn an_edit_made_on_our_series_before_a_clean_merge_lands_on_the_merge() {
    let base = trace(&[1.0]);
    let mut ours = base.clone();
    ours.apply(&Edit(vec![grown(&ours, &[1.0, 2.0])]));
    let mut theirs = base.clone();
    theirs.apply(&Edit(vec![grown(&theirs, &[1.0, 3.0])]));
    let late = Edit(vec![grown(&ours, &[1.0, 2.0, 5.0])]);

    let (mut merged, conflicts) = Document::merge(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    assert_eq!(*merged.root().points, [1.0, 2.0, 3.0]);
    merged.apply(&late);
    assert!(
        merged.root().points.contains(&5.0),
        "the late edit lands: {:?}",
        merged.root().points
    );
}
