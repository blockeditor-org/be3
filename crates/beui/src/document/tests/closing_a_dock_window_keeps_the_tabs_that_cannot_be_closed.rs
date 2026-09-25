use super::*;
use crate::unstyled::{DockDrop, DockState, TabId, dock_state};

#[test]
fn closing_a_dock_window_keeps_the_tabs_that_cannot_be_closed() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let mut layout = DockState::new((1..=3).map(TabId::new));
        for tab in [2, 3] {
            layout.drop_tab(
                TabId::new(tab),
                DockDrop::Window {
                    pos: pos2(200.0, 200.0),
                },
            );
        }
        let second = layout.find(TabId::new(2)).expect("tab 2 is in a window");
        layout.drop_tab(
            TabId::new(3),
            DockDrop::Tab {
                leaf: second.leaf,
                index: 1,
            },
        );
        let (state, set_state) = create_signal(layout);
        view! {
            <styled::DockArea
                @node_ref=&built
                state={state}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                closable={Func::new(|tab: TabId| tab != TabId::new(3))}
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
    let state = dock_state(harness.document(), dock);
    assert_eq!(state.windows().len(), 1, "tabs 2 and 3 share a window");
    let window = state
        .window_rect(state.windows()[0])
        .expect("the window has a rect");
    let tab = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));

    harness.click(pos2(window.right() - 18.0, tab.center().y));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    assert!(!state.contains(TabId::new(2)), "the closable tab is closed");
    assert!(
        state.contains(TabId::new(3)),
        "the tab that cannot be closed stays"
    );
}
