use super::*;
use crate::styled::text_input_value;
use crate::unstyled::{TabId, dock_state};

#[test]
fn ctrl_tab_walks_the_tabs_of_the_pane_the_focus_is_in() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let (state, set_state) = create_signal(unstyled::DockState::new([
            TabId::new(1),
            TabId::new(2),
            TabId::new(3),
        ]));
        view! {
            <styled::DockArea
                @node_ref=&built
                state={state}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: unstyled::DockState| set_state.set(next)}
                on_close={move |_: TabId| {}}
            >
                {move |tab: TabId| {
                    let (typed, set_typed) = create_signal(String::new());
                    view! {
                        <styled::TextInput
                            @test_id={format!("input.{}", tab.value())}
                            value={typed}
                            label="Notes"
                            on_change={move |text: String| set_typed.set(text)}
                        />
                    }
                }}
            </styled::DockArea>
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
