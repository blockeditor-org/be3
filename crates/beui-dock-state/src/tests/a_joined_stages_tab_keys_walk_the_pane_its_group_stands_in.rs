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
fn a_joined_stages_tab_keys_walk_the_pane_its_group_stands_in() {
    let tab = TabId::new;
    let mut state = DockState::default();
    state.reconcile(&spec(pane("main", &[1, 2])));
    state
        .host_group(
            tab(2),
            &joined_key(tab(2)),
            &split(
                "joined:2/root",
                Direction::Horizontal,
                0.7,
                bare("joined:2/stage", &[10, 12]),
                pane("joined:2/side", &[11]),
            ),
            None,
        )
        .expect("the editor's dock, a stage beside a side pane, joins tab 2");
    state
        .host_group(
            tab(10),
            &joined_key(tab(10)),
            &bare("joined:10/stage", &[20]),
            None,
        )
        .expect("a dock inside the editor's stage joins tab 10");
    let main = leaf_of(&state, 1);

    assert_eq!(
        state.standing_leaf(main),
        main,
        "a plain pane walks its own tabs"
    );
    assert_eq!(
        state.standing_leaf(leaf_of(&state, 12)),
        main,
        "the bare stage beside a side pane walks the pane its group took tab 2's place in"
    );
    assert_eq!(
        state.standing_leaf(leaf_of(&state, 20)),
        main,
        "a stage inside a stage walks out through every join"
    );
    let side = leaf_of(&state, 11);
    assert_eq!(
        state.standing_leaf(side),
        side,
        "a joined group's side pane walks its own tabs"
    );
}
