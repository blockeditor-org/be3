use super::*;
use beui::accesskit::{Role, Toggled};

#[test]
fn a_menu_grid_ticks_the_layout_last_picked() {
    let mut test = alone(WIDE, Page::Menus);
    reveal(&mut test, "demo.slide");
    test.click("demo.slide");
    test.frame(Vec::new());
    test.click("demo.slide.1");
    test.settle();
    test.click("demo.slide");
    test.frame(Vec::new());
    test.snapshot("a_menu_grid_ticks_the_layout_last_picked");

    let document = test.document();
    let view = document.accessibility_view().expect("an accessibility tree");
    let ticks: Vec<(String, Toggled)> = view
        .nodes()
        .filter(|node| node.role() == Role::MenuItemRadio)
        .filter_map(|node| Some((node.label()?.to_owned(), node.toggled()?)))
        .collect();
    assert_eq!(ticks.len(), 6, "every layout tile is a radio item: {ticks:?}");
    assert_eq!(
        ticks
            .iter()
            .filter(|(_, toggled)| *toggled == Toggled::True)
            .map(|(label, _)| label.as_str())
            .collect::<Vec<_>>(),
        ["Title and body"],
        "only the picked layout is ticked"
    );
}
