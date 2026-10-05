use super::*;
use crate::reactive::ForEach;
use crate::styled::text_input_value;
use crate::unstyled::{DockPane, DockTab, DockingLayout, TabId, dock_state};

#[test]
fn ctrl_tab_walks_the_tabs_of_the_pane_the_focus_is_in() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let layout = DockingLayout::new();
        view! {
            <styled::Docking @node_ref=&built layout>
                <DockPane id="tabs">
                    <ForEach keys={vec![1u64, 2, 3]}>
                        {move |id: u64| view! {
                            <DockTab
                                id
                                title={format!("Tab {id}")}
                                content={move || {
                                    let (typed, set_typed) = create_signal(String::new());
                                    view! {
                                        <styled::TextInput
                                            @test_id={format!("input.{id}")}
                                            value={typed}
                                            label="Notes"
                                            on_change={move |text: String| set_typed.set(text)}
                                        />
                                    }
                                }}
                            />
                        }}
                    </ForEach>
                </DockPane>
            </styled::Docking>
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
        text_input_value(harness.document(), input),
        "hello",
        "the input took what was typed into it"
    );

    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let leaf = state.leaves(state.main())[0];
    assert_eq!(
        state.active_tab(leaf),
        Some(TabId::new(2)),
        "ctrl+tab shows the next tab even while an input has the focus"
    );
    assert_eq!(
        text_input_value(harness.document(), input),
        "hello",
        "the input the focus was in is left as it was"
    );

    harness.key(Key::Tab, Modifiers::CTRL);
    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());
    assert_eq!(
        dock_state(harness.document(), dock).active_tab(leaf),
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
        dock_state(harness.document(), dock).active_tab(leaf),
        Some(TabId::new(3)),
        "ctrl+shift+tab walks the other way"
    );
}
