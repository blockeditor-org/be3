use super::*;
use crate::unstyled::{DockState, Entry, Side, TabId, dock_state};

#[test]
fn dragging_a_panes_grip_moves_every_tab_of_the_pane() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let mut start = DockState::new([TabId::new(1), TabId::new(2)]);
        let left = start.leaves(start.main())[0];
        start.split(left, Side::Right, 0.5, vec![TabId::new(3)]);
        let (state, set_state) = create_signal(start);
        view! {
            <styled::DockArea
                @node_ref=&built
                state={state}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: DockState| set_state.set(next)}
                on_close={move |_: TabId| {}}
            >
                {move |tab: TabId| view! {
                    <Frame @test_id={format!("content.{}", tab.value())} />
                }}
            </styled::DockArea>
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
