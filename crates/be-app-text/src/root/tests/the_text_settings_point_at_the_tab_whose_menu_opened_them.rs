use super::editor;
use beui::{Rect, pos2};

#[test]
fn the_text_settings_point_at_the_tab_whose_menu_opened_them() {
    let tab = Rect::from_min_max(pos2(24.0, 0.0), pos2(184.0, 30.0));
    let mut editor = editor("hello").with_top_bar(false).in_tab(Some(tab));

    editor.pick_menu("text.settings");

    editor.snapshot("the_text_settings_point_at_the_tab_whose_menu_opened_them");
}
