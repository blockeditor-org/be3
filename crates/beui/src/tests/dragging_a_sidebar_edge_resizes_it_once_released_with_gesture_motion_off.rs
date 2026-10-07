use super::*;
use crate::Motion;
use crate::geometry::vec2;
use crate::unstyled::{DockPane, DockTab, DockWindow, DockingLayout, SIDEBAR_WIDTH, dock_state};

#[test]
fn dragging_a_sidebar_edge_resizes_it_once_released_with_gesture_motion_off() {
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
                    </DockPane>
                </DockWindow>
            </styled::Docking>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.context().set_motion(Motion::Still);
    harness.frame(Vec::new());
    let before = dock_state(harness.document(), dock);
    let leaf = before.leaves(before.windows()[0])[0];

    let panel = harness.rect(harness.find("content.2"));
    let edge = pos2(panel.left() - 3.0, panel.center().y);
    let to = edge + vec2(60.0, 0.0);
    harness.frame(vec![Event::PointerMoved(edge)]);
    harness.frame(vec![Event::PointerButton {
        pos: edge,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![Event::PointerMoved(to)]);

    assert_eq!(
        dock_state(harness.document(), dock).sidebar_width(leaf),
        SIDEBAR_WIDTH,
        "the sidebar holds its width while its edge is held"
    );

    harness.frame(vec![Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());

    assert_eq!(
        dock_state(harness.document(), dock).sidebar_width(leaf),
        SIDEBAR_WIDTH + 60.0,
        "letting go resizes it to where the edge was dragged"
    );
}
