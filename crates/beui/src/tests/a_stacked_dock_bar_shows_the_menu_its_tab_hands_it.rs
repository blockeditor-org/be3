use super::*;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout, dock_menu};
use crate::reactive::*;

const PLAIN: u64 = 1;

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn a_stacked_dock_bar_shows_the_menu_its_tab_hands_it() {
    let _root = crate::reactive::RootScope::new();
    let renamed = create_signal(0);
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking layout focus=2u64>
                <DockPane id="tabs">
                    <DockTab id=PLAIN title="Tab 1">
                        <Frame @test_id="content.1" />
                    </DockTab>
                    <DockTab
                        id=2u64
                        title="Tab 2"
                        content={move || {
                            let rename = Action::new("rename", "Rename", move || {
                                renamed.update(|renamed| *renamed += 1)
                            })
                            .detached();
                            dock_menu(create_memo(move || vec![MenuNode::action(&rename)]));
                            view! {
                                <Frame @test_id="content.2" />
                            }
                        }}
                    />
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, VIEWPORT);
    harness.settle();

    assert!(
        laid_out(&harness, "dock.menu"),
        "the tab handed the bar a menu"
    );
    let at = harness.center(harness.find("dock.menu"));
    harness.click(at);
    harness.settle();
    let root = harness.document().root().expect("the document has a root");
    let row = text_within(harness.document(), root, "Rename").expect("the menu lists Rename");
    harness.click(harness.center(row));
    harness.settle();
    assert_eq!(
        renamed.get_untracked(),
        1,
        "choosing Rename runs the tab's action"
    );

    let switch = harness.center(harness.find("dock.switch"));
    harness.click(switch);
    harness.settle();
    let plain = harness.center(harness.find("dock.switcher.tab.1"));
    harness.click(plain);
    harness.settle();

    assert!(laid_out(&harness, "content.1"));
    assert!(
        !laid_out(&harness, "dock.menu"),
        "the other tab handed the bar no menu"
    );
}
