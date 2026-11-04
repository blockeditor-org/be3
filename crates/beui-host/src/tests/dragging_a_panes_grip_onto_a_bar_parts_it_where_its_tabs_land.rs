use super::*;
use crate::dock::unstyled::{DockApi, DockPane, DockSplit, DockTab, DockingLayout, Entry, TabId};

#[test]
fn dragging_a_panes_grip_onto_a_bar_parts_it_where_its_tabs_land() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <DockSplit id="split">
                    <DockPane id="left">
                        <DockTab id=1u64 title="Tab 1">
                            <Frame @test_id="content.1" />
                        </DockTab>
                        <DockTab id=2u64 title="Tab 2">
                            <Frame @test_id="content.2" />
                        </DockTab>
                    </DockPane>
                    <DockPane id="right">
                        <DockTab id=3u64 title="Tab 3">
                            <Frame @test_id="content.3" />
                        </DockTab>
                        <DockTab id=4u64 title="Tab 4">
                            <Frame @test_id="content.4" />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let second = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let label = harness.rect(dock_tab(harness.document(), dock, "Tab 3"));
    let grip = pos2(label.left() - 25.0, label.center().y);
    let over = pos2(second.left() - 4.0, second.center().y);

    harness.press_at(grip);
    for step in 1..=6 {
        let at = grip + (over - grip) * (step as f32 / 6.0);
        harness.frame(vec![Input::PointerMoved(at)]);
    }
    harness.frames_to_settle();

    assert!(
        harness
            .rect(dock_tab(harness.document(), dock, "Tab 2"))
            .left()
            > second.left(),
        "the bar opens a gap where the pane's tabs would land"
    );

    harness.release_at(over);
    harness.frames_to_settle();

    let state = harness.document().api::<DockApi>(dock).state();
    let leaves = state.leaves(state.main());
    assert_eq!(
        leaves.len(),
        1,
        "the pane dragged away leaves no room behind"
    );
    assert_eq!(
        state.entries(leaves[0]),
        [1, 3, 4, 2].map(|id| Entry::Tab(TabId::new(id))).to_vec(),
        "the pane's tabs land in the gap, in their order"
    );
}
