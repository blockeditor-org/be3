use super::*;
use crate::joined_key;

fn bare(key: &str, tabs: &[u64]) -> DockSpecNode {
    let mut node = pane(key, tabs);
    if let DockSpecNode::Pane(pane) = &mut node {
        pane.bare = true;
    }
    node
}

#[test]
fn a_stage_stands_in_for_the_tab_of_every_joined_dock_around_it() {
    let tab = TabId::new;
    let mut state = DockState::default();
    state.reconcile(&spec(pane("main", &[1, 2])));
    state
        .host_group(
            tab(2),
            &joined_key(tab(2)),
            &bare("joined:2/editor", &[10]),
            None,
        )
        .expect("the editor's dock joins tab 2");
    state
        .host_group(
            tab(10),
            &joined_key(tab(10)),
            &bare("joined:10/stage", &[20]),
            None,
        )
        .expect("a dock inside the editor's stage joins tab 10");

    assert_eq!(
        state.standing_in(tab(20)),
        tab(2),
        "the innermost stage stands in for the outermost tab, through every join"
    );
    assert_eq!(
        state.standing_in(tab(1)),
        tab(1),
        "a plain tab stands for itself"
    );
}
