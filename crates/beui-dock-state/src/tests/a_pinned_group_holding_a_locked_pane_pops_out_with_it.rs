use super::*;

#[test]
fn a_pinned_group_holding_a_locked_pane_pops_out_with_it() {
    let mut state = DockState::default();
    let inner = split(
        "inner",
        Direction::Horizontal,
        0.5,
        locked("locked", &[10]),
        pane("free", &[11]),
    );
    let main = DockSpecNode::Pane(DockSpecPane {
        entries: vec![
            DockSpecEntry::Tab(TabId::new(1)),
            DockSpecEntry::Group {
                key: "nested".to_owned(),
                pinned: true,
                root: Box::new(inner),
            },
        ],
        ..pane_spec("main")
    });
    state.reconcile(&spec(main));
    let group = state.find_group("nested").expect("the group is open");
    let window = DockDrop::Window {
        pos: Pos2::new(40.0, 40.0),
    };

    assert!(
        !state.admits(Entry::Tab(TabId::new(10)), window),
        "the locked tab alone cannot leave its area"
    );
    assert!(
        state.admits(Entry::Group(group), window),
        "the group carries the locked pane, area and all"
    );

    state.drop_entry(Entry::Group(group), window);
    let floated = state.windows()[0];
    assert_eq!(
        state.surface_tabs(floated),
        vec![TabId::new(10), TabId::new(11)]
    );
    assert!(
        state.at_home(TabId::new(10)),
        "the locked tab is still in its area"
    );
}
