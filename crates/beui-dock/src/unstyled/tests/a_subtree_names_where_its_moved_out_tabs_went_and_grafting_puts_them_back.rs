use super::*;
use crate::unstyled::state::{DockMapping, DockOutsidePlace};

#[test]
fn a_subtree_names_where_its_moved_out_tabs_went_and_grafting_puts_them_back() {
    let mut state = DockState::default();
    let main = split(
        "root",
        Direction::Horizontal,
        0.5,
        DockSpecNode::Pane(DockSpecPane {
            entries: vec![DockSpecEntry::Group {
                key: "nested".to_owned(),
                pinned: true,
                root: Box::new(pane("x/inner", &[11, 12])),
            }],
            ..pane_spec("left")
        }),
        pane("right", &[2]),
    );
    state.reconcile(&spec(main.clone()));
    let group = state.find_group("nested").expect("the group is open");
    state.set_tab_pinned(TabId::new(12), false);
    let right = leaf_of(&state, 2);
    state.drop_tab(
        TabId::new(12),
        DockDrop::Tab {
            leaf: right,
            index: 1,
        },
    );
    assert_eq!(state.find(TabId::new(12)).map(|at| at.leaf), Some(right));

    let inner = |tab: TabId| (tab.value() >= 10).then(|| TabId::new(tab.value() - 10));
    let outside = state.outside_of(group, &inner);
    assert_eq!(outside.len(), 1, "the moved-out tab is named");
    assert_eq!(outside[0].tab, TabId::new(2));
    assert_eq!(
        outside[0].place,
        DockOutsidePlace::Pane {
            key: "right".to_owned(),
            index: 1
        }
    );
    let mut subtree = state.subtree(
        group,
        &DockMapping {
            tab: &inner,
            key: &|key| key.strip_prefix("x/").map(str::to_owned),
            group: &|_| None,
        },
    );
    subtree.set_outside(outside);

    let mut fresh = DockState::default();
    fresh.reconcile(&spec(main));
    let group = fresh.find_group("nested").expect("the group is open");
    assert_eq!(fresh.group_tabs(group).len(), 2);
    fresh.graft(
        group,
        &subtree,
        &DockMapping {
            tab: &|tab| Some(TabId::new(tab.value() + 10)),
            key: &|key| Some(format!("x/{key}")),
            group: &|_| None,
        },
    );
    let right = leaf_of(&fresh, 2);
    assert_eq!(
        fresh.find(TabId::new(12)).map(|at| at.leaf),
        Some(right),
        "grafting puts the moved-out tab back in the pane it was in"
    );
    assert!(
        !fresh.is_tab_pinned(TabId::new(12)),
        "and leaves it unlocked"
    );
    assert_eq!(fresh.group_tabs(group), vec![TabId::new(11)]);
}
