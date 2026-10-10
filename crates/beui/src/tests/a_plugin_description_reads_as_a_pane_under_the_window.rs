use accesskit::{Role, Toggled};

use super::*;
use beui_core::accessibility::{Described, embedded, tree_update};
use beui_core::app::accessibility_dump::AccessibilityDump;

fn described(depth: usize, role: Role, label: &str, rect: Rect) -> Described {
    Described {
        depth,
        role,
        label: label.to_owned(),
        value: String::new(),
        toggled: None,
        disabled: false,
        focused: false,
        rect: Some(rect),
        actions: Vec::new(),
    }
}

#[test]
fn a_plugin_description_reads_as_a_pane_under_the_window() {
    let region = Rect::from_min_size(pos2(100.0, 50.0), vec2(200.0, 100.0));
    let mut checked = described(
        1,
        Role::CheckBox,
        "Buy milk",
        Rect::from_min_size(pos2(110.0, 60.0), vec2(120.0, 20.0)),
    );
    checked.toggled = Some(Toggled::True);
    let nodes = [
        described(
            0,
            Role::List,
            "Tasks",
            Rect::from_min_size(pos2(100.0, 50.0), vec2(200.0, 80.0)),
        ),
        checked,
        described(
            0,
            Role::Button,
            "Hidden",
            Rect::from_min_size(pos2(110.0, 400.0), vec2(60.0, 20.0)),
        ),
    ];
    let fragment = embedded(7, "Checklist", region, &nodes);
    let mut dump = AccessibilityDump::new();
    dump.update(tree_update("Test", VIEWPORT, 2.0, vec![fragment]));

    assert_eq!(
        dump.text(),
        "Window \"Test\" focused at 0,0 size 800x600\n\
         \x20 Pane \"Checklist\" at 200,100 size 400x200\n\
         \x20   List \"Tasks\" at 200,100 size 400x160\n\
         \x20     CheckBox \"Buy milk\" toggled=True at 220,120 size 240x40\n\
         \x20   Button \"Hidden\" at 220,800 size 120x40 offscreen\n"
    );
}
