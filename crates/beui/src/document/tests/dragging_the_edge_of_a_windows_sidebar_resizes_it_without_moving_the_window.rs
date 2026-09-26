use super::*;
use crate::geometry::vec2;
use crate::unstyled::{DockState, MIN_SIDEBAR_WIDTH, SIDEBAR_WIDTH, TabId, dock_state};

#[test]
fn dragging_the_edge_of_a_windows_sidebar_resizes_it_without_moving_the_window() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let mut start = DockState::new([TabId::new(1)]);
        let window = start.open_window(
            Rect::from_min_size(pos2(80.0, 60.0), vec2(520.0, 320.0)),
            vec![TabId::new(2)],
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
    let before = dock_state(harness.document(), dock);
    let window = before.windows()[0];
    let leaf = before.leaves(window)[0];
    let placed = before.window_rect(window);
    assert_eq!(before.sidebar_width(leaf), SIDEBAR_WIDTH);

    let panel = harness.rect(harness.find("content.2"));
    let edge = pos2(panel.left() - 3.0, panel.center().y);
    harness.drag(edge, edge + vec2(60.0, 0.0));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    assert_eq!(
        state.sidebar_width(leaf),
        SIDEBAR_WIDTH + 60.0,
        "the sidebar grows by as far as its edge was dragged"
    );
    assert_eq!(
        state.window_rect(window),
        placed,
        "dragging the edge of the sidebar leaves the window where it was"
    );
    let moved = harness.rect(harness.find("content.2"));
    assert_eq!(
        moved.left(),
        panel.left() + 60.0,
        "the panel gives up the room the sidebar took"
    );

    let edge = pos2(moved.left() - 3.0, moved.center().y);
    harness.drag(edge, edge - vec2(400.0, 0.0));
    harness.frame(Vec::new());
    assert_eq!(
        dock_state(harness.document(), dock).sidebar_width(leaf),
        MIN_SIDEBAR_WIDTH,
        "the sidebar stops shrinking at its narrowest"
    );
}
