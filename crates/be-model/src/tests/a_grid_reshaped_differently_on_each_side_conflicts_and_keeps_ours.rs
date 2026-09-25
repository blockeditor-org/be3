use super::*;

#[test]
fn a_grid_reshaped_differently_on_each_side_conflicts_and_keeps_ours() {
    let base = picture(Bounds::new(0, 0, 4, 4));
    let mut ours = base.clone();
    ours.apply(
        &Picture::PIXELS
            .reshape(ObjectId::ROOT, Bounds::new(0, 0, 6, 4))
            .into(),
    );
    let mut theirs = base.clone();
    theirs.apply(
        &Picture::PIXELS
            .reshape(ObjectId::ROOT, Bounds::new(0, 0, 4, 6))
            .into(),
    );

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    assert_eq!(merged.root().pixels.bounds(), Bounds::new(0, 0, 6, 4));
}
