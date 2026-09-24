use super::*;
use block_client::blocks::logic_grid::LogicGrid;

#[test]
fn the_source_link_names_the_grid_it_was_compiled_from() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let grid = client.create_block(LogicGrid::default());
    grid.set_name("Half adder");
    let mut editor = editor_on(client, grid.id());
    editor.run();

    assert_eq!(editor.label("compiled-logic.source"), "Half adder");
}
