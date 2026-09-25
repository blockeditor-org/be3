use super::*;

#[test]
#[ignore = "a merge drops a cell painted outside the other side's crop without counting a conflict"]
fn a_paint_the_other_side_cropped_away_counts_as_a_conflict() {
    let base = picture(Bounds::new(0, 0, 4, 4));
    let mut ours = base.clone();
    ours.apply(
        &Picture::PIXELS
            .reshape(ObjectId::ROOT, Bounds::new(0, 0, 2, 2))
            .into(),
    );
    let theirs = painted(&base, &[(3, 3, 7)]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(merged.root().pixels.bounds(), Bounds::new(0, 0, 2, 2));
    assert_eq!(conflicts, 1);
}
