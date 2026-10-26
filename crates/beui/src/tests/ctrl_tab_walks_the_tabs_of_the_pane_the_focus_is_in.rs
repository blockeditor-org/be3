use super::*;
use crate::dock::unstyled::{DockApi, DockPane, DockTab, DockingLayout, TabId};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn ctrl_tab_walks_the_tabs_of_the_pane_the_focus_is_in() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <DockPane id="tabs">
                    <ForEach keys={vec![1u64, 2, 3]}>|id: u64|
                        <DockTab
                            id
                            title={format!("Tab {id}")}
                            content={move || {
                                let typed = create_signal(String::new());
                                view! {
                                    <styled::TextInput
                                        @test_id={format!("input.{id}")}
                                        value={typed}
                                        label="Notes"
                                    />
                                }
                            }}
                        />
                    </ForEach>
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let input = harness.find("input.1");
    harness.click(harness.center(input));
    harness.type_text("hello");
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().api::<TextInputApi>(input).value(),
        "hello",
        "the input took what was typed into it"
    );

    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    assert_eq!(
        state.active_tab(leaf),
        Some(TabId::new(2)),
        "ctrl+tab shows the next tab even while an input has the focus"
    );
    assert_eq!(
        harness.document().api::<TextInputApi>(input).value(),
        "hello",
        "the input the focus was in is left as it was"
    );

    harness.key(Key::Tab, Modifiers::CTRL);
    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .active_tab(leaf),
        Some(TabId::new(1)),
        "walking past the last tab comes back to the first"
    );

    harness.key(
        Key::Tab,
        Modifiers {
            shift: true,
            ..Modifiers::CTRL
        },
    );
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .active_tab(leaf),
        Some(TabId::new(3)),
        "ctrl+shift+tab walks the other way"
    );
}
