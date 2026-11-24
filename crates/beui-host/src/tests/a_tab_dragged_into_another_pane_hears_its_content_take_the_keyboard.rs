use super::*;
use crate::dock::{DockPane, DockSplit, DockTab, DockTabControl, DockingLayout, use_dock_tab};
use crate::dock_host::unstyled::DockApi;
use crate::reactive::*;

/// A tab moved into another pane, shown there, and hidden behind another tab of it still shows
/// again when a view of its content takes the keyboard once it is in front.
#[test]
fn a_tab_dragged_into_another_pane_hears_its_content_take_the_keyboard() {
    let _root = crate::reactive::RootScope::new();
    let controls = create_signal(Vec::<(u64, DockTabControl)>::new());
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        let tab = move |id: u64| {
            view! {
                <DockTab
                    id
                    title={format!("Tab {id}")}
                    content={move || {
                        if let Some(control) = use_dock_tab() {
                            controls.update(|controls| controls.push((id, control)));
                        }
                        view! {
                            <Frame @test_id={format!("content.{id}")} />
                        }
                    }}
                />
            }
        };
        view! {
            <dock_host::styled::DockHost @node_ref=&dock>
                <dock::Docking layout>
                    <DockSplit id="split">
                        <DockPane id="left">{tab(1)}{tab(2)}</DockPane>
                        <DockPane id="right">{tab(3)}{tab(4)}</DockPane>
                    </DockSplit>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let from = harness.center(harness.find("dock.tab.3"));
    let onto = harness.center(harness.find("dock.tab.2"));
    harness.drag(from, onto);
    harness.settle();
    harness.click(harness.center(harness.find("dock.tab.4")));
    harness.settle();
    let control = controls
        .get_untracked()
        .into_iter()
        .find(|(held, _)| *held == 3)
        .map(|(_, control)| control)
        .expect("tab 3's content has its control");
    control.content_focused();
    harness.settle();
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.focused_tab(),
        Some(crate::dock::TabId::new(3)),
        "its content took the keyboard while it was in front, so it keeps the focus"
    );
}
