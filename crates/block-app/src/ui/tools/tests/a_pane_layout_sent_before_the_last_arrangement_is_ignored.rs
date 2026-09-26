use super::*;

#[test]
fn a_pane_layout_sent_before_the_last_arrangement_is_ignored() {
    let mut state = DockState::new([WORKSPACE]);
    let docked = Docked::default();
    apply(&mut state, &docked, Some(&layout(&[1, 2], 0)));
    let group = docked.group.get().expect("the workspace became a group");
    docked.arrangement.set(1);

    apply(&mut state, &docked, Some(&layout(&[1], 0)));

    assert_eq!(
        state.group_tabs(group),
        vec![pane_tab(PaneId(1)), pane_tab(PaneId(2))],
        "a layout that has not seen arrangement 1 cannot undo it"
    );

    apply(&mut state, &docked, Some(&layout(&[1], 1)));

    assert_eq!(state.group_tabs(group), vec![pane_tab(PaneId(1))]);
}
