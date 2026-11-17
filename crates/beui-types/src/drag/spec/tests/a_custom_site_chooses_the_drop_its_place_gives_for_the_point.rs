use super::*;

#[test]
fn a_custom_site_chooses_the_drop_its_place_gives_for_the_point() {
    let root = RootScope::new();
    let node = origin(&root);
    // Frames along x, one per 10 points, taken only in the first 100.
    let spec = custom(node, |at: Pos2| {
        (at.x < 100.0).then(|| (at.x / 10.0) as u32)
    });

    let over = frame(&spec, at(pos2(42.0, 5.0)), &DragFrame::default());
    assert_eq!(over.drop, Some(4), "the drop its place gives for the point");
    assert!(
        over.made,
        "worked out from the point, so it can change on the same place"
    );
    assert!(over.frame.floating, "the item floats under the pointer");
    assert_eq!(over.frame.item.center(), pos2(42.0, 5.0));

    let moved = frame(&spec, at(pos2(57.0, 5.0)), &over.frame);
    assert_eq!(
        moved.frame.chosen, over.frame.chosen,
        "the same place stays chosen"
    );
    assert_eq!(moved.drop, Some(5), "with the drop the new point gives");

    let off = frame(&spec, at(pos2(150.0, 5.0)), &moved.frame);
    assert_eq!(off.drop, None, "where it gives none, nothing is taken");
}
