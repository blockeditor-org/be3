use super::*;
use crate::dock::unstyled::{DockApi, DockPane, DockTab, DockingLayout, TabId};
use crate::reactive::*;

#[test]
fn closing_a_dock_tab_asks_its_owner_to_remove_it() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let closed = create_signal(Vec::new());
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <DockPane id="tabs">
                    <ForEach keys={vec![1u64, 2, 3]}>|id: u64|
                        <DockTab
                            id
                            title={format!("Tab {id}")}
                            on_close={move || closed.update(|closed| closed.push(TabId::new(id)))}
                        >
                            <Frame @test_id={format!("content.{id}")} />
                        </DockTab>
                    </ForEach>
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));

    harness.frame(vec![Input::PointerMoved(tab)]);
    harness.down(tab, PointerButton::Middle);
    harness.up(tab, PointerButton::Middle);
    harness.frame(Vec::new());

    assert_eq!(
        closed.get_untracked(),
        vec![TabId::new(2)],
        "its owner is asked"
    );
    assert_eq!(
        harness.document().api::<DockApi>(dock).state().all_tabs(),
        vec![TabId::new(1), TabId::new(2), TabId::new(3)],
        "the tab stays until its owner removes it"
    );
}
