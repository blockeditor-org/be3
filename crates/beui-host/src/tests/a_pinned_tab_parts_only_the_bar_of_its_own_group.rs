use super::*;
use crate::dock::unstyled::{DockGroup, DockPane, DockTab, DockingLayout};

#[test]
fn a_pinned_tab_parts_only_the_bar_of_its_own_group() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking layout focus=10u64>
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
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the dock was built");
    let label =
        |harness: &Harness, title: &str| harness.rect(dock_tab(harness.document(), root, title));
    let pinned = label(&harness, "Tab 10").center();
    let outside = label(&harness, "Tab 1");
    let inside = label(&harness, "Tab 11");

    harness.press_at(pinned);
    harness.frame(vec![Input::PointerMoved(outside.center())]);
    harness.frames_to_settle();

    assert_eq!(
        label(&harness, "Tab 1"),
        outside,
        "a bar outside the pinned tab's group opens no gap for it"
    );
    assert!(
        harness.document().find_test_id("dock.drop").is_none(),
        "nor does the dock show it anywhere else to land"
    );

    let past = pos2(inside.right() + 20.0, inside.center().y);
    harness.frame(vec![Input::PointerMoved(past)]);
    harness.frames_to_settle();

    assert!(
        label(&harness, "Tab 11").left() < inside.left() - 10.0,
        "in its own group's bar the tab it passes glides aside"
    );
    harness.release_at(past);
    harness.frames_to_settle();
}
