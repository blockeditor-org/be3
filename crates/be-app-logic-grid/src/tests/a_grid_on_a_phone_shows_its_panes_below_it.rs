use super::*;

#[test]
fn a_grid_on_a_phone_shows_its_panes_below_it() {
    let mut editor = sized(Vec2::new(390.0, 844.0));
    editor.settle();

    assert!(
        editor.shown("logic-grid.toolbar") && editor.shown("logic-grid.hotbar"),
        "a phone shows the grid with its toolbars and its panes gathered below it"
    );
    editor.snapshot("a_grid_on_a_phone_shows_its_panes_below_it");
}
