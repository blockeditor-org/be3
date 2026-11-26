use super::editor;
use be_sdk_test::prelude::*;

/// A row with fewer cells than the table draws its closing pipe after its own last cell, where
/// the header's pipe between those columns is, not at the table's right edge.
#[test]
fn a_short_table_row_closes_after_its_own_last_cell() {
    let mut editor = editor("|a|b|c|\n|-|-|-|\n|e|\n");
    editor.run();

    editor.snapshot("a_short_table_row_closes_after_its_own_last_cell");
}
