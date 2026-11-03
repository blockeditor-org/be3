use super::*;

#[test]
fn a_bare_pane_shows_its_one_tab_and_takes_no_other() {
    let mut state = DockState::default();
    let mut stage = pane("stage", &[1]);
    if let DockSpecNode::Pane(pane) = &mut stage {
        pane.bare = true;
    }
    state.reconcile(&spec(split(
        "root",
        Direction::Horizontal,
        0.5,
        stage,
        pane("tools", &[2]),
    )));
    let stage = leaf_of(&state, 1);
    assert!(state.is_bare(stage));
    assert!(!state.is_bare(leaf_of(&state, 2)));
    let two = Entry::Tab(TabId::new(2));
    assert!(!state.admits(two, DockDrop::Pane { leaf: stage }));
    assert!(!state.admits(
        two,
        DockDrop::Tab {
            leaf: stage,
            index: 1
        }
    ));
    assert!(!state.admits(
        two,
        DockDrop::Group {
            leaf: stage,
            index: 0
        }
    ));
    assert!(
        state.admits(
            two,
            DockDrop::Split {
                leaf: stage,
                side: Side::Right
            }
        ),
        "a bare pane can still be split from its edge"
    );
    state.drop_tab(TabId::new(2), DockDrop::Pane { leaf: stage });
    assert_eq!(state.entries(stage), vec![Entry::Tab(TabId::new(1))]);
    let restored = DockState::from_spec(&state.spec());
    assert!(
        restored.is_bare(leaf_of(&restored, 1)),
        "a restored layout carries a bare pane"
    );
}
