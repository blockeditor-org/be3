use super::*;
use crate::dock::{DockPane, DockTab, DockTabControl, DockingLayout, TabId, use_dock_tab};
use crate::dock_host::unstyled::DockApi;
use crate::primitives::*;
use crate::reactive::*;

/// A view of a tab's content that takes the keyboard after its pane hid the tab (an app putting
/// it back on its popup's trigger as a press on another tab closes the popup) leaves the tab
/// shown in its place.
#[test]
fn a_hidden_tabs_content_taking_the_keyboard_late_leaves_the_tab_shown_in_its_place() {
    let _root = crate::reactive::RootScope::new();
    let controls = create_signal(Vec::<(u64, DockTabControl)>::new());
    let dock = NodeRef::new();
    let layout = DockingLayout::<u64>::new();
    let document = build_node(move || {
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
                            <Frame />
                        }
                    }}
                />
            }
        };
        view! {
            <dock_host::styled::DockHost @node_ref=&dock>
                <dock::Docking layout focus=1u64>
                    <DockPane id="tabs">{tab(1)}{tab(2)}</DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let control = |id: u64| {
        controls
            .get_untracked()
            .into_iter()
            .find(|(held, _)| *held == id)
            .map(|(_, control)| control)
            .expect("each tab's content has its control")
    };
    let active = |harness: &Harness| {
        let state = harness.document().api::<DockApi>(dock.get()).state();
        let leaf = state.leaves(state.main())[0];
        state.active_tab(leaf)
    };
    control(1).content_focused();
    harness.settle();

    harness.click(harness.center(harness.find("dock.tab.2")));
    harness.settle();
    assert_eq!(active(&harness), Some(TabId::new(2)));
    control(1).content_focused();
    harness.settle();
    assert_eq!(
        active(&harness),
        Some(TabId::new(2)),
        "the hidden tab's late keyboard leaves the tab pressed shown"
    );

    harness.click(harness.center(harness.find("dock.tab.1")));
    harness.settle();
    control(1).content_focused();
    harness.settle();
    assert_eq!(active(&harness), Some(TabId::new(1)), "shown again, its content counts again");
}
