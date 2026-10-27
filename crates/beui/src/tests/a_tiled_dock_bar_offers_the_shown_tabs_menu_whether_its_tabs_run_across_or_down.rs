use super::*;
use crate::dock::unstyled::{DockApi, DockPane, DockTab, DockingLayout, dock_menu};
use crate::reactive::*;

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

fn choose(harness: &mut Harness, label: &str) {
    let at = harness.center(harness.find("dock.tabs.menu"));
    harness.click(at);
    harness.settle();
    let root = harness.document().root().expect("the document has a root");
    let row = text_within(harness.document(), root, label)
        .unwrap_or_else(|| panic!("the menu lists {label}"));
    harness.click(harness.center(row));
    harness.settle();
}

#[test]
fn a_tiled_dock_bar_offers_the_shown_tabs_menu_whether_its_tabs_run_across_or_down() {
    let _root = crate::reactive::RootScope::new();
    let ran = create_signal(0);
    let dock = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking @node_ref=&dock layout focus=2u64>
                <DockPane id="tabs">
                    <DockTab id=1u64 title="Tab 1">
                        <Frame @test_id="content.1" />
                    </DockTab>
                    <DockTab
                        id=2u64
                        title="Tab 2"
                        content={move || {
                            let tidy = Action::new("tidy", "Tidy up", move || {
                                ran.update(|ran| *ran += 1)
                            })
                            .detached();
                            dock_menu(create_memo(move || vec![MenuNode::action(&tidy)]));
                            view! {
                                <Frame @test_id="content.2" />
                            }
                        }}
                    />
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let dock = dock.get();

    assert!(
        laid_out(&harness, "dock.tabs.menu"),
        "the bar offers the shown tab's menu"
    );
    let menu = harness.rect(harness.find("dock.tabs.menu"));
    let tab = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    assert!(
        menu.left() > tab.right(),
        "the menu sits at the end of the bar, past the tabs"
    );
    choose(&mut harness, "Tidy up");
    assert_eq!(
        ran.get_untracked(),
        1,
        "choosing an item runs the tab's action"
    );

    let label = harness.rect(dock_tab(harness.document(), dock, "Tab 1"));
    let grip = pos2(label.left() - 25.0, label.center().y);
    harness.frame(vec![Input::PointerMoved(grip)]);
    harness.frame(vec![Input::PointerButton {
        pos: grip,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());
    let sidebar = text_within(harness.document(), dock, "Show tabs in a sidebar")
        .expect("the grip's menu offers a sidebar");
    harness.click(harness.center(sidebar));
    harness.settle();
    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    assert!(
        state.is_vertical(leaf),
        "the pane shows its tabs in a sidebar"
    );
    assert!(
        laid_out(&harness, "dock.tabs.menu"),
        "a pane with its tabs in a sidebar still offers the menu"
    );
    choose(&mut harness, "Tidy up");
    assert_eq!(ran.get_untracked(), 2);

    let first = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    harness.click(first);
    harness.settle();
    assert!(laid_out(&harness, "content.1"));
    assert!(
        !laid_out(&harness, "dock.tabs.menu"),
        "a tab that hands over no menu shows no menu button"
    );
}
