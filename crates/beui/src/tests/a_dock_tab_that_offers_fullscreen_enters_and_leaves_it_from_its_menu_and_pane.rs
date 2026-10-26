use super::*;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout, use_dock_tab};
use crate::reactive::*;

#[component]
fn Offered() -> NodeId {
    if let Some(tab) = use_dock_tab() {
        tab.offer_fullscreen();
    }
    view! {
        <Frame @test_id="offered" />
    }
}

fn right_click(harness: &mut Harness, dock: NodeId, title: &str) {
    let pos = harness.center(dock_tab(harness.document(), dock, title));
    harness.frame(vec![Input::PointerMoved(pos)]);
    for pressed in [true, false] {
        harness.frame(vec![Input::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed,
        }]);
    }
    harness.frame(Vec::new());
}

fn pick(harness: &mut Harness, dock: NodeId, row: &str) {
    let row = text_within(harness.document(), dock, row)
        .unwrap_or_else(|| panic!("the menu offers {row:?}"));
    harness.click(harness.center(row));
    harness.settle();
}

#[test]
fn a_dock_tab_that_offers_fullscreen_enters_and_leaves_it_from_its_menu_and_pane() {
    let _root = crate::reactive::RootScope::new();
    let layout = DockingLayout::<u64>::new();
    let dock = NodeRef::new();
    let document = {
        build_node(move || {
            view! {
                <dock::styled::Docking @node_ref=&dock layout>
                    <DockPane id="tabs">
                        <DockTab id=1u64 title="Offered">
                            <Offered />
                        </DockTab>
                        <DockTab id=2u64 title="Plain">
                            <Frame @test_id="plain" />
                        </DockTab>
                    </DockPane>
                </dock::styled::Docking>
            }
        })
    };
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let docked = harness.rect(harness.find("offered"));

    right_click(&mut harness, dock, "Plain");
    assert!(
        text_within(harness.document(), dock, "Full screen").is_none(),
        "a tab whose content does not offer fullscreen has no such entry"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.settle();

    right_click(&mut harness, dock, "Offered");
    pick(&mut harness, dock, "Full screen");
    assert_eq!(
        layout.fullscreen(),
        Some(1),
        "the entry makes the tab fullscreen"
    );

    let screen = Rect::from_min_size(
        pos2(0.0, WIDE_VIEWPORT.y - 100.0),
        vec2(WIDE_VIEWPORT.x, 100.0),
    );
    layout.enter_fullscreen(&1, Some(screen));
    harness.settle();
    assert!(
        harness.rect(harness.find("dock.away.leave")).max.y <= screen.min.y,
        "the pane it left offers the way back where the fullscreen area does not cover it"
    );
    right_click(&mut harness, dock, "Offered");
    pick(&mut harness, dock, "Leave full screen");
    assert_eq!(layout.fullscreen(), None, "the entry turns into leaving it");

    layout.enter_fullscreen(&1, Some(screen));
    harness.settle();
    harness.click(harness.center(harness.find("dock.away.leave")));
    harness.settle();
    assert_eq!(
        layout.fullscreen(),
        None,
        "the button in the pane it left brings it back"
    );
    assert!(harness.document().find_test_id("dock.away").is_none());
    assert_eq!(harness.rect(harness.find("offered")), docked);
}
