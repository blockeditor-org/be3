use super::*;
use crate::unstyled::{DockPane, DockTab, DockingLayout, TabId, dock_state};

#[component]
fn TwoTabs(layout: DockingLayout<u64>) -> NodeId {
    view! {
        <styled::Docking layout>
            <DockPane id="tabs">
                <DockTab id=1u64 title="Tab 1">
                    <Frame @test_id="content.1" />
                </DockTab>
                <DockTab id=2u64 title="Tab 2">
                    <Frame @test_id="content.2" />
                </DockTab>
            </DockPane>
        </styled::Docking>
    }
}

fn docked(layout: DockingLayout<u64>) -> (Harness, NodeId) {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        view! {
            <TwoTabs @node_ref=&built layout={layout.clone()} />
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    (harness, dock.get())
}

#[test]
fn a_layout_put_back_from_its_snapshot_keeps_the_tabs_where_they_were() {
    let first = DockingLayout::new();
    let (mut harness, dock) = docked(first.clone());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    harness.drag(tab, pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());
    let saved = first.snapshot();
    assert_eq!(
        saved.state().leaves(saved.state().main()).len(),
        2,
        "the snapshot holds the split the user made"
    );

    let second = DockingLayout::new();
    second.restore(saved);
    let (reopened, dock) = docked(second);

    let state = dock_state(reopened.document(), dock);
    let leaves = state.leaves(state.main());
    assert_eq!(leaves.len(), 2, "the restored dock is split as it was");
    assert_eq!(
        state.find(TabId::new(2)).map(|position| position.leaf),
        Some(leaves[1]),
        "the tab dragged to the edge is back in the pane it was dropped in"
    );
}
