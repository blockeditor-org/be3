use super::*;
use crate::dock::unstyled::{DockApi, DockPane, DockTab, DockWindow, DockingLayout};

const WINDOW: Rect = Rect::from_min_max(pos2(40.0, 40.0), pos2(940.0, 560.0));
const SMALLER: Vec2 = Vec2::new(800.0, 420.0);

#[test]
fn a_window_larger_than_the_dock_is_drawn_no_larger_than_it() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout>
                <DockPane id="tabs">
                    <DockTab id=1u64 title="Tab 1">
                        <Frame @test_id="content.1" />
                    </DockTab>
                </DockPane>
                <DockWindow id="window" rect=WINDOW>
                    <DockPane id="floating">
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
    let window = harness.document().api::<DockApi>(dock).state().windows()[0];
    let stored = harness
        .document()
        .api::<DockApi>(dock)
        .state()
        .window_rect(window)
        .expect("the window has a rect");
    let content = |harness: &Harness| harness.rect(harness.find("content.2"));
    let chrome = stored.size() - content(&harness).size();

    harness.resize(SMALLER);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let shrunk = harness.rect(dock);
    let drawn = content(&harness);
    assert!(
        drawn.width() + chrome.x <= shrunk.width() && drawn.height() + chrome.y <= shrunk.height(),
        "the window fits inside the smaller dock: {drawn:?} in {shrunk:?}"
    );
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .window_rect(window),
        Some(stored),
        "the window keeps the size it was given for when the dock grows again"
    );
}
