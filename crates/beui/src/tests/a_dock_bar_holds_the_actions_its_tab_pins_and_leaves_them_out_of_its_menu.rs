use super::*;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout, dock_menu};
use crate::reactive::*;

const PHONE: Vec2 = Vec2::new(390.0, 600.0);

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn a_dock_bar_holds_the_actions_its_tab_pins_and_leaves_them_out_of_its_menu() {
    let _root = crate::reactive::RootScope::new();
    let shared = create_signal(0);
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking layout focus=1u64>
                <DockPane id="tabs">
                    <DockTab
                        id=1u64
                        title="Notes"
                        content={move || {
                            let share = Action::new("share", "Share", move || {
                                shared.update(|shared| *shared += 1)
                            })
                            .pin(Pin::Label)
                            .detached();
                            let rename = Action::new("rename", "Rename", || {}).detached();
                            dock_menu(create_memo(move || {
                                vec![MenuNode::action(&rename), MenuNode::action(&share)]
                            }));
                            view! {
                                <Frame @test_id="content.1" />
                            }
                        }}
                    />
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let root = harness.document().root().expect("the document has a root");
    let pin = harness.find("dock.tabs.pin.share");
    assert!(
        text_within(harness.document(), pin, "Share").is_some(),
        "a tiled bar labels the pinned action"
    );
    harness.click(harness.center(pin));
    harness.settle();
    assert_eq!(shared.get_untracked(), 1, "pressing it runs the action");

    let at = harness.center(harness.find("dock.tabs.menu"));
    harness.click(at);
    harness.settle();
    assert!(text_within(harness.document(), root, "Rename").is_some());
    assert!(
        harness
            .document()
            .find_test_id("dock.tabs.menu.share")
            .is_none(),
        "the menu leaves out what the bar already holds"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.settle();

    harness.resize(PHONE);
    harness.settle();
    assert!(
        !laid_out(&harness, "dock.pin.share"),
        "a stacked bar keeps only the actions pinned as icons"
    );
    let at = harness.center(harness.find("dock.menu"));
    harness.click(at);
    harness.settle();
    assert!(
        laid_out(&harness, "dock.menu.share"),
        "and its menu offers the one pinned with a label"
    );
}
