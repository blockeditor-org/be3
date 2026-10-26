use super::*;

#[test]
fn a_spec_seeds_the_layout_it_describes() {
    let mut state = DockState::default();
    let mut editors = pane("editors", &[2, 3]);
    if let DockSpecNode::Pane(editors) = &mut editors {
        editors.active = Some(TabId::new(3));
    }
    let mut spec = spec(split(
        "root",
        Direction::Horizontal,
        0.3,
        pane("files", &[1]),
        editors,
    ));
    spec.windows.push(DockSpecWindow {
        key: "float".to_owned(),
        rect: Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(300.0, 200.0)),
        root: Some(pane("floating", &[4])),
    });
    state.reconcile(&spec);

    let leaves = state.leaves(state.main());
    assert_eq!(leaves.len(), 2, "the split lays out both of its panes");
    assert_eq!(
        state.entries(leaves[0]),
        vec![Entry::Tab(TabId::new(1))],
        "the first pane holds the tab declared in it"
    );
    assert_eq!(
        state.active_tab(leaves[1]),
        Some(TabId::new(3)),
        "a pane starts on the tab it names as active"
    );
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1006.0, 400.0));
    let splitter = layout_surface(&state, state.main(), area, 6.0).splitters[0];
    assert_eq!(
        state.split_fraction(splitter.id),
        0.3,
        "the split starts at the fraction the code gave it"
    );
    let windows = state.windows();
    assert_eq!(windows.len(), 1, "a declared window floats over the dock");
    assert_eq!(
        state.surface_tabs(windows[0]),
        vec![TabId::new(4)],
        "the window holds the tab declared in it"
    );
    assert_eq!(
        state.window_rect(windows[0]).map(|rect| rect.min),
        Some(Pos2::new(10.0, 20.0)),
        "the window opens where the code put it"
    );
}
