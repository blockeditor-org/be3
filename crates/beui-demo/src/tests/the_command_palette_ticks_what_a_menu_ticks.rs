use super::*;
use beui::accesskit::{Role, Toggled};

fn ticks(test: &DocumentTest) -> Vec<(String, Toggled)> {
    let document = test.document();
    let Some(view) = document.accessibility_view() else {
        return Vec::new();
    };
    view.nodes()
        .filter(|node| node.role() == Role::ListBoxOption)
        .filter_map(|node| Some((node.label()?.to_owned(), node.toggled()?)))
        .collect()
}

#[test]
fn the_command_palette_ticks_what_a_menu_ticks() {
    let mut test = alone(WIDE, Page::Menus);
    reveal(&mut test, "demo.palette.open");
    test.click("demo.palette.open");
    test.settle();
    let ticks = ticks(&test);
    for (label, ticked) in [
        ("Wrap lines", Toggled::True),
        ("Small", Toggled::False),
        ("Medium", Toggled::True),
        ("Large", Toggled::False),
    ] {
        assert!(
            ticks.contains(&(label.to_owned(), ticked)),
            "the palette row {label} reads as {ticked:?}, as its menu row does: {ticks:?}"
        );
    }
    test.snapshot("the_command_palette_ticks_checked_and_chosen_actions");
}
