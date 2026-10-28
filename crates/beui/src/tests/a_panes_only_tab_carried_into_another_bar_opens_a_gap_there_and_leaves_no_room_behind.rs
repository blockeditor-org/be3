use super::*;
use crate::dock::unstyled::{DockApi, DockPane, DockSplit, DockTab, DockingLayout, Entry, TabId};

#[test]
fn a_panes_only_tab_carried_into_another_bar_opens_a_gap_there_and_leaves_no_room_behind() {
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
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let second = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let from = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let over = pos2(second.left(), second.center().y);

    harness.press_at(from);
    for step in 1..=6 {
        let at = from + (over - from) * (step as f32 / 6.0);
        harness.frame(vec![Input::PointerMoved(at)]);
    }
    harness.frames_to_settle();

    let parted = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    assert!(
        parted.left() > second.left() + second.width() / 2.0,
        "the tab the carried one would land before glides aside: {} became {}",
        second.left(),
        parted.left()
    );

    harness.release_at(over);
    harness.frames_to_settle();

    let state = harness.document().api::<DockApi>(dock).state();
    let leaves = state.leaves(state.main());
    assert_eq!(leaves.len(), 1, "the emptied pane leaves no room behind");
    assert_eq!(
        state.entries(leaves[0]),
        [1, 3, 2].map(|id| Entry::Tab(TabId::new(id))).to_vec(),
        "the tab lands in the gap it opened"
    );
    harness.find("content.3");
}
