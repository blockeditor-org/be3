use super::*;
use beui::unstyled::DockDrop;

#[test]
fn a_pane_moved_out_of_the_workspace_stays_out_when_the_plugin_rearranges() {
    let mut state = DockState::new([WORKSPACE]);
    let docked = Docked::default();
    apply(&mut state, &docked, Some(&layout(&[1, 2], 0)));
    let group = docked.group.get().expect("the workspace became a group");
    state.drop_tab(
        pane_tab(PaneId(2)),
        DockDrop::Window {
            pos: pos2(30.0, 30.0),
        },
    );

    let moved = arranged(&state, group);
    assert_eq!(moved.detached, vec![PaneId(2)]);

    apply(&mut state, &docked, Some(&layout(&[1, 2, 3], 0)));

    assert_eq!(
        state.group_tabs(group),
        vec![pane_tab(PaneId(1)), pane_tab(PaneId(3))]
    );
    assert_eq!(state.windows().len(), 1, "the popped out pane keeps its window");

    apply(&mut state, &docked, Some(&layout(&[1, 3], 0)));

    assert!(
        !state.contains(pane_tab(PaneId(2))),
        "a pane the plugin no longer lists is closed wherever it is"
    );
}
