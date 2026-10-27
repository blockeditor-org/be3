use super::editor;
use block_ui_test::prelude::*;

#[test]
fn a_table_rule_is_drawn_only_where_its_row_has_a_pipe() {
    let mut editor = editor("| abcde | b | c |\n| - | - | - |\n| e \n");
    editor.run();

    editor.snapshot("a_table_rule_is_drawn_only_where_its_row_has_a_pipe");
}
