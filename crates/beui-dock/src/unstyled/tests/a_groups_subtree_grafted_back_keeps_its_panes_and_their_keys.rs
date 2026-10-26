use super::*;
use crate::unstyled::state::DockMapping;

#[test]
fn a_groups_subtree_grafted_back_keeps_its_panes_and_their_keys() {
    let mut state = DockState::default();
    let inner = split(
        "inner",
        Direction::Horizontal,
        0.5,
        locked("x/locked", &[10]),
        pane("x/free", &[11, 12]),
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
    let out = DockMapping {
        tab: &|tab| (tab.value() >= 10).then(|| TabId::new(tab.value() - 10)),
        key: &|key| key.strip_prefix("x/").map(str::to_owned),
        group: &|_| None,
    };

    let mut subtree = state.subtree(group, &out);
    assert_eq!(
        subtree.all_tabs(),
        vec![TabId::new(0), TabId::new(1), TabId::new(2)],
        "the subtree names the group's tabs as the mapping does"
    );
    let free = leaf_of(&subtree, 1);
    assert_eq!(subtree.pane_key(free), Some("free"));
    assert_eq!(
        subtree.home(TabId::new(0)),
        Some(DockHome::Area("locked".to_owned()))
    );

    subtree.drop_tab(
        TabId::new(2),
        DockDrop::Split {
            leaf: free,
            side: Side::Below,
        },
    );
    let back = DockMapping {
        tab: &|tab| Some(TabId::new(tab.value() + 10)),
        key: &|key| Some(format!("x/{key}")),
        group: &|_| None,
    };
    state.graft(group, &subtree, &back);

    let leaves = state.tree_leaves(Tree::Group(group));
    assert_eq!(leaves.len(), 3, "the split made in the subtree is in the group");
    assert_eq!(state.pane_key(leaf_of(&state, 11)), Some("x/free"));
    assert_eq!(state.area_of(leaf_of(&state, 10)), Some("x/locked"));
    assert_eq!(
        state.home(TabId::new(10)),
        Some(DockHome::Area("x/locked".to_owned()))
    );
    assert_eq!(state.home(TabId::new(12)), Some(DockHome::Group(group)));
    assert!(state.contains(TabId::new(1)));
}
