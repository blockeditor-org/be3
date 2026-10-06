use super::*;
use crate::geometry::vec2;
use crate::unstyled::{DockPane, DockTab, DockWindow, DockingLayout, dock_state};

#[test]
fn a_window_with_tabs_in_a_sidebar_has_no_title_bar() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let layout = DockingLayout::new();
        view! {
            <styled::Docking @node_ref=&built layout>
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
                        <DockTab id=3u64 title="Tab 3">
                            <Frame @test_id="content.3" />
                        </DockTab>
                    </DockPane>
                </DockWindow>
            </styled::Docking>
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
