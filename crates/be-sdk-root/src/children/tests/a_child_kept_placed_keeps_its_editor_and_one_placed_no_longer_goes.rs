use super::*;

#[test]
fn a_child_kept_placed_keeps_its_editor_and_one_placed_no_longer_goes() {
    let place = rect((0.0, 0.0), (100.0, 100.0));
    let mut other = placed(place, ChildMode::Live);
    other.child = ChildId(8);
    if let ChildContent::Block { block_id, .. } = &mut other.content {
        *block_id = [9; 16];
    }
    let first = reconcile(
        Vec::new(),
        vec![
            wanted(placed(place, ChildMode::Live)),
            wanted(other.clone()),
        ],
        &[],
    );
    assert_eq!(first.opened, vec![0, 1], "each child placed is opened");
    assert!(first.gone.is_empty());
    let kept = Rc::clone(&first.kept[0].shared);

    // Placed again at another place, the first keeps its editor; the second is placed no
    // longer and goes.
    let moved = rect((10.0, 10.0), (110.0, 110.0));
    let second = reconcile(
        first.kept,
        vec![wanted(placed(moved, ChildMode::Live))],
        &[],
    );
    assert!(
        second.opened.is_empty(),
        "a child placed again is not opened again"
    );
    assert!(Rc::ptr_eq(&second.kept[0].shared, &kept));
    assert_eq!(second.kept[0].placement.rect, moved);
    assert_eq!(second.gone.len(), 1);
    assert_eq!(second.gone[0].child, ChildId(8));

    // Placed naming another editor for the same block, it is another editor: a twin of its
    // own, since the place it held the block's editor in goes with this.
    let mut named = placed(moved, ChildMode::Live);
    if let ChildContent::Block { editor, .. } = &mut named.content {
        *editor = Some([3; 16]);
    }
    let third = reconcile(second.kept, vec![wanted(named)], &[]);
    assert_eq!(third.opened, vec![0]);
    assert_eq!(third.gone.len(), 1);

    // A block open above its placer is refused, never opened.
    let refused = reconcile(
        Vec::new(),
        vec![wanted(placed(place, ChildMode::Live))],
        &[[1; 16]],
    );
    assert!(refused.opened.is_empty());
    assert!(matches!(
        &*refused.kept[0].shared.showing.borrow(),
        Showing::Failed(why) if why == ALREADY_OPEN_ABOVE
    ));

    // Only a block child placed to run its editor, or its preview, is shown.
    assert!(
        Child::placed(
            EditorInstanceId(1),
            ViewId::MAIN,
            1.0,
            placed(place, ChildMode::Hidden),
            Vec::new(),
        )
        .is_none()
    );
}
