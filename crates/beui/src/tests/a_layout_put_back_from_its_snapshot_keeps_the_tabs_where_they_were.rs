use super::*;
use crate::dock::unstyled::{DockApi, DockPane, DockTab, DockingLayout, TabId};

#[component]
fn TwoTabs(layout: DockingLayout<u64>) -> NodeId {
    view! {
        <dock::styled::Docking layout>
            <DockPane id="tabs">
                <DockTab id=1u64 title="Tab 1">
                    <Frame @test_id="content.1" />
                </DockTab>
                <DockTab id=2u64 title="Tab 2">
                    <Frame @test_id="content.2" />
                </DockTab>
            </DockPane>
        </dock::styled::Docking>
    }
}

fn docked(layout: DockingLayout<u64>) -> (Harness, NodeId) {
    let dock = NodeRef::new();
    let document = build_node(move || {
        view! {
            <TwoTabs @node_ref=&dock layout={layout} />
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    (harness, dock.get())
}

#[test]
fn a_layout_put_back_from_its_snapshot_keeps_the_tabs_where_they_were() {
    let _root = crate::reactive::RootScope::new();
    let first = DockingLayout::new();
    let (mut harness, dock) = docked(first);
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

    let state = reopened.document().api::<DockApi>(dock).state();
    let leaves = state.leaves(state.main());
    assert_eq!(leaves.len(), 2, "the restored dock is split as it was");
    assert_eq!(
        state.find(TabId::new(2)).map(|position| position.leaf),
        Some(leaves[1]),
        "the tab dragged to the edge is back in the pane it was dropped in"
    );
}
