use super::*;
use crate::reactive::*;
use crate::dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout};

#[test]
fn a_dock_stacked_by_narrowing_it_leaves_the_hidden_panel_unplaced() {
    let _root = crate::reactive::RootScope::new();
    let home = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking layout focus=2u64>
                <DockSplit id="split">
                    <DockPane id="left">
                        <DockTab id=1u64 title="Tab 1">
                            <Frame @node_ref=&home />
                        </DockTab>
                    </DockPane>
                    <DockPane id="right">
                        <DockTab id=2u64 title="Tab 2">
                            <Frame />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let home = home.get();
    assert!(
        harness.document().node_rect(home).is_some(),
        "the tiled dock lays out the home tab beside the other"
    );

    harness.resize(VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().node_rect(home),
        None,
        "the stacked dock shows the other tab, so the home tab's panel has no place on screen"
    );
}
