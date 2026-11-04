use super::*;

#[test]
fn the_menu_key_opens_a_dock_tabs_menu_beneath_the_tab() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("dock.tab.2")));
    harness.frame(Vec::new());
    for _ in 0..8 {
        if holds_tab(&harness) {
            break;
        }
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
    }
    assert!(holds_tab(&harness), "Tab reaches the second tab");
    let tab = harness.rect(harness.find("dock.tab.2"));

    for (key, modifiers) in [
        (Key::ContextMenu, Modifiers::NONE),
        (Key::F10, Modifiers::SHIFT),
    ] {
        harness.key(key, modifiers);
        harness.frame(Vec::new());
        let row = text_within(harness.document(), dock, "Pop out into a window")
            .unwrap_or_else(|| panic!("{key:?} opens the focused tab's menu"));
        let row = harness.rect(row);
        assert!(
            row.min.y >= tab.max.y - 1.0,
            "the menu opens beneath the tab: {row:?} under {tab:?}"
        );
        harness.key(Key::Escape, Modifiers::NONE);
        harness.frame(Vec::new());
        assert!(text_within(harness.document(), dock, "Pop out into a window").is_none());
    }

    harness.key(Key::ContextMenu, Modifiers::NONE);
    harness.frame(Vec::new());
    let close = text_within(harness.document(), dock, "Close tab").expect("the menu is open");
    harness.click(harness.center(close));
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("dock.tab.2").is_none(),
        "the menu the key opened is the focused tab's"
    );
    assert!(harness.document().find_test_id("dock.tab.1").is_some());
}

fn holds_tab(harness: &Harness) -> bool {
    let document = harness.document();
    let Some(focused) = document.focused_node() else {
        return false;
    };
    let mut next = Some(harness.find("dock.tab.2"));
    while let Some(node) = next {
        if node == focused {
            return true;
        }
        next = document.arena.parent(node);
    }
    false
}
