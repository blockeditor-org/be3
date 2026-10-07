use super::*;
use crate::unstyled::{DockGroup, DockPane, DockTab, DockingLayout};

fn shown(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some_and(|rect| rect.width() > 0.0 && rect.height() > 0.0)
}

#[test]
fn a_drop_the_dock_would_refuse_draws_no_drop_marker() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let layout = DockingLayout::new();
        view! {
            <styled::Docking @node_ref=&built layout focus=10u64>
                <DockPane id="main">
                    <DockTab id=1u64 title="Tab 1">
                        <Frame @test_id="content.1" />
                    </DockTab>
                    <DockGroup id="pinned" pinned=true>
                        <DockPane id="inside">
                            <DockTab id=10u64 title="Tab 10">
                                <Frame @test_id="content.10" />
                            </DockTab>
                            <DockTab id=11u64 title="Tab 11">
                                <Frame @test_id="content.11" />
                            </DockTab>
                        </DockPane>
                    </DockGroup>
                </DockPane>
            </styled::Docking>
        }
    });
    dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the dock was built");
    let pinned = harness.center(dock_tab(harness.document(), root, "Tab 10"));
    let outside = harness.center(dock_tab(harness.document(), root, "Tab 1"));
    let inside = harness.center(dock_tab(harness.document(), root, "Tab 11"));

    harness.press_at(pinned);
    harness.frame(vec![Event::PointerMoved(outside)]);
    harness.frame(Vec::new());

    assert!(
        !shown(&harness, "dock.drop"),
        "a pinned tab over a bar outside its group is shown no place to land"
    );

    harness.frame(vec![Event::PointerMoved(inside)]);
    harness.frame(Vec::new());

    assert!(
        shown(&harness, "dock.drop"),
        "the same tab over its own group's bar is shown where it would land"
    );
    harness.release_at(inside);
}
