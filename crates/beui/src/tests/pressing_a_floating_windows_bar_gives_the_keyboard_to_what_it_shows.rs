use super::*;
use crate::styled::*;
use crate::dock::unstyled::{DockPane, DockTab, DockWindow, DockingLayout};

#[component]
fn Input(id: u64) -> NodeId {
    let typed = create_signal(String::new());
    view! {
        <styled::TextInput @test_id={format!("input.{id}")} value={typed} label="Notes" />
    }
}

#[test]
fn pressing_a_floating_windows_bar_gives_the_keyboard_to_what_it_shows() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <DockPane id="main">
                    <DockTab id=1u64 title="Main">
                        <Input id=1 />
                    </DockTab>
                </DockPane>
                <DockWindow
                    id="window"
                    rect={Rect::from_min_size(pos2(500.0, 100.0), vec2(400.0, 300.0))}
                >
                    <DockPane id="floating">
                        <DockTab id=2u64 title="Floating">
                            <Input id=2 />
                        </DockTab>
                    </DockPane>
                </DockWindow>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let (main, floating) = (harness.find("input.1"), harness.find("input.2"));
    let tab = harness.rect(dock_tab(harness.document(), dock, "Floating"));
    let bar = pos2(tab.max.x + 120.0, tab.center().y);

    harness.click(harness.rect(main).min + vec2(40.0, 40.0));
    assert!(harness.document().focus_is_within(main));
    harness.drag(bar, bar + vec2(-30.0, 40.0));
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(floating),
        "pressing a window's bar gives the keyboard to the window"
    );
    harness.type_text("moved");
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().api::<TextInputApi>(floating).value(),
        "moved"
    );

    let bar = bar + vec2(-30.0, 40.0);
    harness.click(bar);
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(floating),
        "pressing the bar of the window that has the keyboard keeps it there"
    );
    let tab = harness.rect(dock_tab(harness.document(), dock, "Floating"));
    harness.click(tab.center());
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(floating),
        "pressing the window's own tab keeps the keyboard in what it shows"
    );
    harness.type_text(" again");
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().api::<TextInputApi>(floating).value(),
        "moved again"
    );
    assert_eq!(harness.document().api::<TextInputApi>(main).value(), "");
}
