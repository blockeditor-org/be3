use super::*;
use crate::unstyled::{DockPane, DockSplit, DockTab, DockingLayout, Entry, TabId, dock_state};

#[test]
fn dragging_a_panes_grip_moves_every_tab_of_the_pane() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let layout = DockingLayout::new();
        view! {
            <styled::Docking @node_ref=&built layout>
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
                    </DockPane>
                </DockSplit>
            </styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let label = harness.rect(dock_tab(harness.document(), dock, "Tab 1"));
    let grip = pos2(label.left() - 25.0, label.center().y);
    let onto = harness.center(harness.find("content.3"));
    harness.drag(grip, onto);
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let leaves = state.leaves(state.main());
    assert_eq!(
        leaves.len(),
        1,
        "the pane dragged away leaves no room behind"
    );
    assert_eq!(
        state.entries(leaves[0]),
        vec![
            Entry::Tab(TabId::new(3)),
            Entry::Tab(TabId::new(1)),
            Entry::Tab(TabId::new(2))
        ],
        "every tab of the pane joins the pane it was dropped on"
    );
    assert_eq!(
        state.active_tab(leaves[0]),
        Some(TabId::new(1)),
        "the pane goes on showing the tab the dragged pane showed"
    );
}
