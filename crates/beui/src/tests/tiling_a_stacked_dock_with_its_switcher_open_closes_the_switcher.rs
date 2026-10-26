use super::*;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout, dock_menu};
use crate::reactive::*;

const PHONE: Vec2 = Vec2::new(390.0, 800.0);

#[component]
fn Docked() -> NodeId {
    let layout = DockingLayout::new();
    view! {
        <dock::styled::Docking layout focus=2u64>
            <DockPane id="tabs">
                <ForEach keys={vec![1u64, 2]}>|id: u64|
                    <DockTab
                        id
                        title={format!("Tab {id}")}
                        on_close={|| {}}
                        content={move || {
                            let act = Action::new(format!("action.{id}"), "Act", || {})
                                    .pin(Pin::Icon)
                                    .detached();
                                dock_menu(create_memo(move || vec![MenuNode::action(&act)]));
                            view! {
                                <Frame @test_id={format!("content.{id}")} />
                            }
                        }}
                    />
                </ForEach>
            </DockPane>
        </dock::styled::Docking>
    }
}

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn tiling_a_stacked_dock_with_its_switcher_open_closes_the_switcher() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <Docked />
        }
    });
    let mut harness = Harness::sized(document, PHONE);
    harness.settle();
    let at = harness.center(harness.find("dock.switch"));
    harness.click(at);
    harness.settle();
    assert!(laid_out(&harness, "dock.switcher.tab.2"));

    harness.resize(WIDE_VIEWPORT);
    harness.settle();
    assert!(
        !laid_out(&harness, "dock.switcher.tab.2"),
        "the switcher goes with the stacked screen"
    );
    assert!(laid_out(&harness, "content.2"));

    harness.resize(PHONE);
    harness.settle();
    assert!(
        laid_out(&harness, "dock.pin.action.2"),
        "stacking the dock again brings the tab's actions back to its bar"
    );
}
