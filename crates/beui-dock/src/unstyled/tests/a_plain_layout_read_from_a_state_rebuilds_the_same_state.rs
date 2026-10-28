use super::*;

#[test]
fn a_plain_layout_read_from_a_state_rebuilds_the_same_state() {
    let mut state = DockState::default();
    let inner = split(
        "inner",
        Direction::Horizontal,
        0.3,
        locked("locked", &[10]),
        pane("free", &[11, 12]),
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
    state.show(TabId::new(12));
    let plain = state.plain();

    let rebuilt = DockState::from_plain(&plain);

    assert_eq!(rebuilt.plain(), plain);
    assert_eq!(rebuilt.focused_tab(), Some(TabId::new(12)));
    assert_eq!(rebuilt.all_tabs(), state.all_tabs());
    let group = rebuilt
        .find_group("nested")
        .expect("the group keeps its key");
    assert!(rebuilt.is_pinned(group));
    assert_eq!(
        rebuilt.home(TabId::new(10)),
        Some(DockHome::Area("locked".to_owned()))
    );
}
