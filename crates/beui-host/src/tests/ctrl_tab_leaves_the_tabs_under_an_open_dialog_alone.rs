use super::*;
use crate::dock::{DockPane, DockTab, DockingLayout};
use crate::dock_host::unstyled::DockApi;
use crate::reactive::*;
use crate::styled::List;
use crate::styled::{Dialog, ListItem, Space, TextInput};

#[test]
fn ctrl_tab_leaves_the_tabs_under_an_open_dialog_alone() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let open = create_signal(false);
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        view! {
            <List spacing=Space::None>
                <ListItem grow=1.0>
                    <dock_host::styled::DockHost @node_ref=&dock>
                        <dock::Docking layout>
                            <DockPane id="tabs">
                                <ForEach value={vec![1u64, 2, 3]}>|id: u64|
                                    <DockTab
                                        id
                                        title={format!("Tab {id}")}
                                        content={move || {
                                            let typed = create_signal(String::new());
                                            view! {
                                                <TextInput
                                                    @test_id={format!("input.{id}")}
                                                    value={typed}
                                                    label="Notes"
                                                />
                                            }
                                        }}
                                    />
                                </ForEach>
                            </DockPane>
                        </dock::Docking>
                    </dock_host::styled::DockHost>
                </ListItem>
                <Dialog open={open} title="Rename" on_dismiss={move || open.set(false)}>
                    <TextInput @test_id={"dialog.name"} value="" label="Name" />
                </Dialog>
            </List>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let input = harness.find("input.1");
    harness.click(harness.center(input));
    harness.frame(Vec::new());
    let active = |harness: &Harness| {
        let state = harness.document().api::<DockApi>(dock).state();
        let leaf = state.leaves(state.main())[0];
        state.active_tab(leaf)
    };
    let shown = active(&harness);

    open.set(true);
    harness.frame(Vec::new());
    let name = harness.find("dialog.name");
    harness.click(harness.center(name));
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());
    assert_eq!(
        active(&harness),
        shown,
        "Ctrl+Tab in a dialog outside the dock leaves the tab under it"
    );

    open.set(false);
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("input.1")));
    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());
    assert_ne!(
        active(&harness),
        shown,
        "with the focus back in the dock it walks again"
    );
}
