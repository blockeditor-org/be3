use super::*;
use crate::dock::unstyled::{
    DockApi, DockPane, DockTab, DockWindow, DockingLayout, MIN_SIDEBAR_WIDTH, SIDEBAR_WIDTH,
};
use crate::geometry::vec2;

#[test]
fn dragging_the_edge_of_a_windows_sidebar_resizes_it_without_moving_the_window() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <DockPane id="main">
                    <DockTab id=1u64 title="Tab 1">
                        <Frame @test_id="content.1" />
                    </DockTab>
                </DockPane>
                <DockWindow
                    id="window"
                    rect={Rect::from_min_size(pos2(80.0, 60.0), vec2(520.0, 320.0))}
                >
                    <DockPane id="side" vertical=true>
                        <DockTab id=2u64 title="Tab 2">
                            <Frame @test_id="content.2" />
                        </DockTab>
                    </DockPane>
                </DockWindow>
            </dock::styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let before = harness.document().api::<DockApi>(dock).state();
    let window = before.windows()[0];
    let leaf = before.leaves(window)[0];
    let placed = before.window_rect(window);
    assert_eq!(before.sidebar_width(leaf), SIDEBAR_WIDTH);

    let panel = harness.rect(harness.find("content.2"));
    let edge = pos2(panel.left() - 3.0, panel.center().y);
    harness.drag(edge, edge + vec2(60.0, 0.0));
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
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
    harness.drag(
        edge,
        edge - vec2(SIDEBAR_WIDTH + 60.0 - MIN_SIDEBAR_WIDTH, 0.0),
    );
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.sidebar_width(leaf),
        MIN_SIDEBAR_WIDTH,
        "the sidebar stops shrinking at its narrowest"
    );
    assert!(!state.is_icons_only(leaf), "and still shows its titles");

    let edge = pos2(
        harness.rect(harness.find("content.2")).left() - 3.0,
        moved.center().y,
    );
    harness.drag(edge, edge - vec2(400.0, 0.0));
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    assert!(
        state.is_icons_only(leaf),
        "dragged on past its narrowest, it shows only the icons of its tabs"
    );
    assert_eq!(
        state.window_rect(window),
        placed,
        "and the window stays where it was"
    );
}
