use super::*;
use crate::geometry::vec2;
use crate::unstyled::{DockState, TabId, dock_state};

#[test]
fn a_window_with_tabs_in_a_sidebar_has_no_title_bar() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let mut start = DockState::new([TabId::new(1)]);
        let window = start.open_window(
            Rect::from_min_size(pos2(80.0, 60.0), vec2(520.0, 320.0)),
            vec![TabId::new(2), TabId::new(3)],
        );
        let leaf = start.leaves(window)[0];
        start.set_vertical(leaf, true);
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

    let tab = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let below = harness.rect(dock_tab(harness.document(), dock, "Tab 3"));
    assert!(below.top() > tab.bottom(), "the window stacks its tabs");
    let panel = harness.rect(harness.find("content.2"));
    assert!(
        panel.left() > tab.right(),
        "the panel sits beside the sidebar"
    );
    assert!(
        panel.top() < tab.top(),
        "the panel reaches the top of the window, with no title bar above it"
    );

    let grip = pos2(tab.left() + 8.0, tab.top() - 16.0);
    harness.frame(vec![Event::PointerMoved(grip)]);
    harness.frame(vec![Event::PointerButton {
        pos: grip,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());
    let row = text_within(harness.document(), dock, "Show tabs across the top")
        .expect("the grip at the top of the sidebar offers the title bar back");
    harness.click(harness.center(row));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let window = state.windows()[0];
    assert!(
        !state.is_vertical(state.leaves(window)[0]),
        "the window shows its tabs across the top again"
    );
    let tab = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let panel = harness.rect(harness.find("content.2"));
    assert!(
        panel.top() > tab.bottom(),
        "the panel sits below the title bar again"
    );
}
