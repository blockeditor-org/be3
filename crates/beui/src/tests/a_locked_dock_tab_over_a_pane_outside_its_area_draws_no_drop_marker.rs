use super::*;
use crate::dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout};

fn shown(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some_and(|rect| rect.width() > 0.0 && rect.height() > 0.0)
}

#[test]
fn a_locked_dock_tab_over_a_pane_outside_its_area_draws_no_drop_marker() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking layout focus=10u64>
                <DockSplit id="workspace" fraction=0.3>
                    <DockPane id="files" locked=true>
                        <DockTab id=1u64 title="Tab 1">
                            <Frame />
                        </DockTab>
                        <DockTab id=2u64 title="Tab 2">
                            <Frame />
                        </DockTab>
                    </DockPane>
                    <DockPane id="editors" locked=true>
                        <DockTab id=10u64 title="Tab 10">
                            <Frame />
                        </DockTab>
                        <DockTab id=11u64 title="Tab 11">
                            <Frame />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the dock was built");
    let locked = harness.center(dock_tab(harness.document(), root, "Tab 10"));
    let outside = harness.center(dock_tab(harness.document(), root, "Tab 1"));
    let inside = harness.center(dock_tab(harness.document(), root, "Tab 11"));

    let first = harness.center(dock_tab(harness.document(), root, "Tab 2"));

    harness.press_at(locked);
    harness.frame(vec![Input::PointerMoved(outside)]);
    harness.frames_to_settle();

    assert!(
        !shown(&harness, "dock.drop"),
        "a locked tab over a bar outside its area is shown no place to land"
    );
    assert_eq!(
        harness.center(dock_tab(harness.document(), root, "Tab 2")),
        first,
        "and that bar opens no gap for it"
    );

    harness.frame(vec![Input::PointerMoved(inside)]);
    harness.frames_to_settle();
    harness.release_at(inside);
    harness.frames_to_settle();

    let carried = harness.center(dock_tab(harness.document(), root, "Tab 10"));
    let passed = harness.center(dock_tab(harness.document(), root, "Tab 11"));
    assert!(
        carried.x > passed.x,
        "the same tab over its own area's bar lands where it was shown, after Tab 11"
    );
}
