use super::*;
use crate::unstyled::{Entry, TabId, dock_state};

fn choose(harness: &mut Harness, dock: NodeId, pos: Pos2, item: &str) {
    harness.frame(vec![Event::PointerMoved(pos)]);
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());
    let row = text_within(harness.document(), dock, item)
        .unwrap_or_else(|| panic!("the grip's menu offers {item}"));
    harness.click(harness.center(row));
    harness.frame(Vec::new());
}

fn grip_beside(harness: &Harness, dock: NodeId, first_tab: &str) -> Pos2 {
    let label = harness.rect(dock_tab(harness.document(), dock, first_tab));
    pos2(label.left() - 25.0, label.center().y)
}

fn grip_above(harness: &Harness, dock: NodeId, first_tab: &str) -> Pos2 {
    let label = harness.rect(dock_tab(harness.document(), dock, first_tab));
    pos2(label.left() + 1.0, label.top() - 20.0)
}

#[test]
fn dock_tabs_moved_into_a_sidebar_stack_beside_the_panel() {
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let grip = grip_beside(&harness, dock, "Tab 1");
    choose(&mut harness, dock, grip, "Show tabs in a sidebar");

    let state = dock_state(harness.document(), dock);
    let leaf = state.leaves(state.main())[0];
    assert!(
        state.is_vertical(leaf),
        "the pane shows its tabs in a sidebar"
    );
    let first = harness.rect(dock_tab(harness.document(), dock, "Tab 1"));
    let second = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    assert!(
        second.top() > first.bottom(),
        "the second tab sits below the first"
    );
    assert_eq!(first.left(), second.left(), "the tabs line up in a column");
    let panel = harness.rect(harness.find("content.1"));
    assert!(
        panel.left() > first.right(),
        "the panel sits beside the sidebar"
    );
    assert!(
        panel.top() < first.bottom(),
        "the panel reaches up beside the tabs, with no bar above it"
    );

    let dragged = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let onto = harness.rect(dock_tab(harness.document(), dock, "Tab 1"));
    harness.drag(dragged, pos2(onto.center().x, onto.top() - 4.0));
    harness.frame(Vec::new());
    assert_eq!(
        dock_state(harness.document(), dock).entries(leaf),
        vec![
            Entry::Tab(TabId::new(3)),
            Entry::Tab(TabId::new(1)),
            Entry::Tab(TabId::new(2))
        ],
        "a tab dropped on the top edge of another in the sidebar lands above it"
    );

    let grip = grip_above(&harness, dock, "Tab 3");
    choose(&mut harness, dock, grip, "Show tabs across the top");

    assert!(
        !dock_state(harness.document(), dock).is_vertical(leaf),
        "the pane shows its tabs across the top again"
    );
    let first = harness.rect(dock_tab(harness.document(), dock, "Tab 3"));
    let second = harness.rect(dock_tab(harness.document(), dock, "Tab 1"));
    assert!(
        second.left() > first.right(),
        "the tabs sit side by side in a bar again"
    );
    let panel = harness.rect(harness.find("content.3"));
    assert!(
        panel.top() > first.bottom(),
        "the panel sits below the bar again"
    );
}
