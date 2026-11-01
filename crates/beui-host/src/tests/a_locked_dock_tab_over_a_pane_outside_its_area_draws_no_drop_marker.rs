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

    harness.down(locked, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(outside)]);
    harness.frame(Vec::new());

    assert!(
        !shown(&harness, "dock.drop"),
        "a locked tab over a bar outside its area is shown no place to land"
    );

    harness.frame(vec![Input::PointerMoved(inside)]);
    harness.frame(Vec::new());

    assert!(
        shown(&harness, "dock.drop"),
        "the same tab over its own area's bar is shown where it would land"
    );
    harness.up(inside, PointerButton::Primary);
}
