use super::*;

#[test]
fn a_grid_grown_the_same_way_on_both_sides_keeps_both_paints() {
    let base = picture(Bounds::new(0, 0, 2, 2));
    let grow: Edit = Picture::PIXELS
        .reshape(ObjectId::ROOT, Bounds::new(0, 0, 4, 4))
        .into();
    let mut ours = base.clone();
    ours.apply(&grow);
    let ours = painted(&ours, &[(3, 3, 1)]);
    let mut theirs = base.clone();
    theirs.apply(&grow);
    let theirs = painted(&theirs, &[(2, 2, 2)]);

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(merged.root().pixels.bounds(), Bounds::new(0, 0, 4, 4));
    assert_eq!(pixel(&merged, 3, 3), Some(1));
    assert_eq!(pixel(&merged, 2, 2), Some(2));
}
