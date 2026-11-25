use super::*;

/// Block 1 placed as child `id` of editor 1's main view, `mode`, at `rect`, naming `editor`.
fn at(id: u64, rect: Rect, mode: ChildMode, editor: Option<[u8; 16]>) -> Child {
    let mut placement = placed(rect, mode);
    placement.child = ChildId(id);
    if let ChildContent::Block { editor: named, .. } = &mut placement.content {
        *named = editor;
    }
    wanted(placement)
}

#[test]
fn a_block_placed_in_several_places_gets_an_editor_in_each() {
    let small = rect((0.0, 0.0), (100.0, 60.0));
    let large = rect((0.0, 100.0), (400.0, 340.0));
    // The block placed live, and previewed small and large: each place opens an editor of its
    // own, as two views of one open block each get their own.
    let first = reconcile(
        Vec::new(),
        vec![
            at(1, small, ChildMode::Live, None),
            at(2, small, ChildMode::Preview, None),
            at(3, large, ChildMode::Preview, None),
        ],
        &[],
    );
    assert_eq!(
        first.opened,
        vec![0, 1, 2],
        "each place opens its own editor"
    );
    assert!(!Rc::ptr_eq(&first.kept[0].shared, &first.kept[1].shared));
    assert!(!Rc::ptr_eq(&first.kept[1].shared, &first.kept[2].shared));
    assert!(
        first
            .kept
            .iter()
            .all(|child| child.takes_input(&HashSet::default())
                == (child.placement.mode == ChildMode::Live))
    );

    // Each shows its own place: the live one its main view, a preview its preview view.
    let live = Rc::clone(&first.kept[0].shared);
    live.asks.borrow_mut().push(PlaceAsk {
        placement: first.kept[0].placement.clone(),
        raster: 1.0,
        framing: None,
        space: None,
    });
    live.show(&Docks::default());
    assert_eq!(
        first.kept[0].screen(EditorInstanceId(9)),
        main_screen(EditorInstanceId(9))
    );
    assert_eq!(
        first.kept[1].screen(EditorInstanceId(9)),
        preview_screen_of(EditorInstanceId(9))
    );

    // Placed again, each place keeps its editor; another place, naming another editor or not,
    // opens one of its own; the places placed no longer go.
    let editors: Vec<Rc<Editor>> = first
        .kept
        .iter()
        .map(|child| Rc::clone(&child.shared))
        .collect();
    let second = reconcile(
        first.kept,
        vec![
            at(1, small, ChildMode::Live, None),
            at(4, small, ChildMode::Live, Some([3; 16])),
            at(6, small, ChildMode::Passive, None),
        ],
        &[],
    );
    assert_eq!(
        second.opened,
        vec![1, 2],
        "only the new places open editors"
    );
    assert!(Rc::ptr_eq(&second.kept[0].shared, &editors[0]));
    assert!(!Rc::ptr_eq(&second.kept[1].shared, &second.kept[2].shared));
    assert_eq!(second.gone.len(), 2, "the two previews placed no longer go");
}
