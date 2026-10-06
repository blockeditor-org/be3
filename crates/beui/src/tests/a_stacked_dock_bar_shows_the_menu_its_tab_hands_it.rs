use super::*;
use crate::reactive::{Action, create_memo};
use crate::unstyled::{DockMode, DockPane, DockTab, DockingLayout, dock_menu};

const HOME: u64 = 1;

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn a_stacked_dock_bar_shows_the_menu_its_tab_hands_it() {
    let renamed = Rc::new(Cell::new(0));
    let renaming = renamed.clone();
    let document = build(move || {
        let layout = DockingLayout::new();
        view! {
            <styled::Docking layout mode=DockMode::Stacked home=HOME focus=2u64>
                <DockPane id="tabs">
                    <DockTab id=HOME title="Tab 1">
                        <Frame @test_id="content.1" />
                    </DockTab>
                    <DockTab
                        id=2u64
                        title="Tab 2"
                        content={move || {
                            let renaming = renaming.clone();
                            let rename = Action::new("rename", "Rename", move || {
                                renaming.set(renaming.get() + 1)
                            })
                            .detached();
                            dock_menu(create_memo(move || vec![rename.clone()]));
                            view! {
                                <Frame @test_id="content.2" />
                            }
                        }}
                    />
                </DockPane>
            </styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
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
    assert_eq!(renamed.get(), 1, "choosing Rename runs the tab's action");

    let back = harness.center(harness.find("dock.back"));
    harness.click(back);
    harness.settle();

    assert!(laid_out(&harness, "content.1"));
    assert!(
        !laid_out(&harness, "dock.menu"),
        "the home tab handed the bar no menu"
    );
}
