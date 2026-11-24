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
fn a_block_placed_in_several_places_shares_one_editor_but_a_twin() {
    let small = rect((0.0, 0.0), (100.0, 60.0));
    let large = rect((0.0, 100.0), (400.0, 340.0));
    // The block placed live, and previewed small and large: one editor shows it in all three,
    // opened once.
    let first = reconcile(
        Vec::new(),
        vec![
            at(1, small, ChildMode::Live, None),
            at(2, small, ChildMode::Preview, None),
            at(3, large, ChildMode::Preview, None),
        ],
        &[],
    );
    assert_eq!(first.opened, vec![0], "the block's editor is opened once");
    let editor = Rc::clone(&first.kept[0].shared);
    assert!(
        first
            .kept
            .iter()
            .all(|child| Rc::ptr_eq(&child.shared, &editor))
    );

    // Once every place said what it asks, its main view is the live place's and its preview
    // view the largest preview's; the small preview shows a copy, hit nowhere.
    for child in &first.kept {
        editor.asks.borrow_mut().push(PlaceAsk {
            key: child.place(),
            placement: child.placement.clone(),
            raster: 1.0,
            framing: None,
        });
    }
    let docks = Docks::default();
    editor.show(&docks);
    assert_eq!(editor.main_place.get(), Some(first.kept[0].place()));
    assert_eq!(editor.preview_place.get(), Some(first.kept[2].place()));
    assert!(!first.kept[0].copy());
    assert!(first.kept[1].copy());
    assert!(!first.kept[2].copy());
    assert!(!first.kept[1].takes_input(&HashSet::default()));
    let copy = copy_layer(LayerId(1), first.kept[1].place());
    assert_ne!(copy, LayerId(1));
    assert_ne!(copy, copy_layer(LayerId(1), first.kept[2].place()));

    // The small place asking for it active (a canvas focusing that card) takes its main view.
    let mut focused = first.kept[1].placement.clone();
    focused.mode = ChildMode::Active;
    editor.asks.borrow_mut()[1].placement = focused;
    editor.show(&docks);
    assert_eq!(editor.main_place.get(), Some(first.kept[1].place()));
    *editor.asks.borrow_mut() = Vec::new();

    // A place asking for another editor than the one holding the block is a twin of its own;
    // a preview asking for one is never a twin, and one asking for none shares the block's.
    let second = reconcile(
        first.kept,
        vec![
            at(1, small, ChildMode::Live, None),
            at(4, small, ChildMode::Live, Some([3; 16])),
            at(5, small, ChildMode::Preview, Some([3; 16])),
            at(6, small, ChildMode::Passive, None),
        ],
        &[],
    );
    assert_eq!(second.opened, vec![1], "only the twin is opened");
    assert!(Rc::ptr_eq(&second.kept[0].shared, &editor));
    assert!(!Rc::ptr_eq(&second.kept[1].shared, &editor));
    assert!(Rc::ptr_eq(&second.kept[2].shared, &editor));
    assert!(Rc::ptr_eq(&second.kept[3].shared, &editor));
    assert_eq!(second.gone.len(), 2, "the two previews placed no longer go");
}
