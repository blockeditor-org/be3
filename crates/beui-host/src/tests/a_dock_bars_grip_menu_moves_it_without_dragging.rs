use super::*;
use crate::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_dock_bars_grip_menu_moves_it_without_dragging() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(
        DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    harness.click(harness.center(harness.find("dock.bar.9.grip")));
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the document has a root");
    let row = text_within(harness.document(), root, "Dock along the bottom")
        .expect("the grip opens a menu of placements");
    harness.click(harness.center(row));
    harness.frame(Vec::new());
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(
            DockEdge::Bottom,
            DockAlign::Center
        ))
    );

    harness.click(harness.center(harness.find("dock.bar.9.grip")));
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the document has a root");
    let row = text_within(harness.document(), root, "Pop out into a window")
        .expect("the menu offers a window");
    harness.click(harness.center(row));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(matches!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Window {
            edge: DockEdge::Bottom,
            ..
        })
    ));
    assert!(harness.document().find_test_id("bar.9").is_some());
}
