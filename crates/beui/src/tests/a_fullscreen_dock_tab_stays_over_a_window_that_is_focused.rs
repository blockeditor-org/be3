use super::*;
use crate::reactive::with_reactive_scope;
use crate::unstyled::{DockPane, DockTab, DockingLayout};

#[test]
fn a_fullscreen_dock_tab_stays_over_a_window_that_is_focused() {
    let _root = crate::reactive::RootScope::new();
    let layout = DockingLayout::<u64>::new();
    let dock = NodeRef::new();
    let document = build(move || {
        view! {
            <styled::Docking @node_ref=&dock layout focus=1u64>
                <DockPane id="tabs">
                    <DockTab id=1u64 title="Tab 1">
                        <Frame @test_id="content.1" />
                    </DockTab>
                    <DockTab id=2u64 title="Tab 2">
                        <Frame @test_id="content.2" />
                    </DockTab>
                    <DockTab id=3u64 title="Tab 3">
                        <Frame @test_id="content.3" />
                    </DockTab>
                </DockPane>
            </styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let dock = dock.get();
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    drag_with(&mut harness, tab, pos2(400.0, 200.0), Modifiers::ALT);
    harness.settle();

    with_reactive_scope(harness.document_mut(), move || {
        layout.enter_fullscreen(&1, None)
    });
    harness.settle();
    with_reactive_scope(harness.document_mut(), move || layout.show(&3));
    harness.settle();

    let document = harness.document();
    let layers = document.layers();
    let layer_of = |node| document.layer_of(&layers, node).expect("in a layer");
    let fullscreen = layer_of(harness.find("dock.fullscreen"));
    let window = layer_of(dock_tab(document, dock, "Tab 3"));
    assert!(
        fullscreen > window,
        "the fullscreen tab covers the window that was focused under it ({fullscreen} over {window})"
    );
}
