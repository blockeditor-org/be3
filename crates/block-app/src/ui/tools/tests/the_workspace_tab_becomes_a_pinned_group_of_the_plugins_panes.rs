use super::*;

#[test]
fn the_workspace_tab_becomes_a_pinned_group_of_the_plugins_panes() {
    let mut state = DockState::new([WORKSPACE]);
    let docked = Docked::default();

    apply(&mut state, &docked, Some(&layout(&[1, 2], 0)));

    let group = docked.group.get().expect("the workspace became a group");
    assert!(state.is_pinned(group));
    assert!(!state.contains(WORKSPACE));
    assert_eq!(
        state.group_tabs(group),
        vec![pane_tab(PaneId(1)), pane_tab(PaneId(2))]
    );

    apply(&mut state, &docked, None);

    assert!(docked.group.get().is_none());
    assert_eq!(
        state.all_tabs(),
        vec![WORKSPACE],
        "without panes the workspace is one tab again"
    );
}
