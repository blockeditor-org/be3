use super::*;

#[test]
fn a_pane_tree_that_ends_early_is_refused() {
    let short = PaneTree {
        items: vec![
            PaneItem::Tabs {
                count: 2,
                active: 0,
                vertical: false,
                sidebar: SIDEBAR_WIDTH,
            },
            PaneItem::Pane(PaneId(1)),
        ],
    };
    let deep = PaneTree {
        items: std::iter::repeat_n(
            PaneItem::Split {
                horizontal: true,
                fraction: 0.5,
            },
            MAX_PANE_DEPTH + 2,
        )
        .collect(),
    };

    assert_eq!(dock_tree(&short), None);
    assert_eq!(dock_tree(&deep), None);
    assert_eq!(dock_tree(&PaneTree::default()), None);
}
