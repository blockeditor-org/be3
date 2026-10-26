use super::*;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout, dock_menu};
use crate::reactive::*;

const PHONE: Vec2 = Vec2::new(390.0, 800.0);

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

fn tap(harness: &mut Harness, test_id: &str) {
    let at = harness.center(harness.find(test_id));
    harness.click(at);
    harness.settle();
}

#[test]
fn a_switcher_card_offers_its_tabs_menu_and_closes_when_an_item_runs() {
    let _root = crate::reactive::RootScope::new();
    let shared = create_signal(Vec::<u64>::new());
    let document = build_node(move || {
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
                                let share = Action::new("share", "Share", move || {
                                    shared.update(|shared| shared.push(id))
                                })
                                .pin(Pin::Label)
                                .detached();
                                dock_menu(create_memo(move || vec![MenuNode::action(&share)]));
                                view! {
                                    <Frame @test_id={format!("content.{id}")} />
                                }
                            }}
                        />
                    </ForEach>
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, PHONE);
    harness.settle();
    tap(&mut harness, "dock.switch");
    tap(&mut harness, "dock.switcher.tab.1");
    tap(&mut harness, "dock.switch");
    assert!(laid_out(&harness, "dock.switcher.menu.2"));

    tap(&mut harness, "dock.switcher.menu.2");
    tap(&mut harness, "dock.switcher.menu.2.share");
    assert_eq!(
        shared.get_untracked(),
        vec![2],
        "the card's menu runs the action of its own tab, even one not on show"
    );
    assert!(
        !laid_out(&harness, "dock.switcher.tab.2"),
        "and the switcher closes, so what the action opens is not behind it"
    );
    assert!(laid_out(&harness, "content.1"), "the tab on show stays");
}
