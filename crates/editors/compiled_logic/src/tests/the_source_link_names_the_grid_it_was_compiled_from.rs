use super::*;
use block_client::blocks::logic_grid::LogicGrid;

#[test]
fn the_source_link_names_the_grid_it_was_compiled_from() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let grid = client.create_block(LogicGrid::default());
    grid.set_name("Half adder");
    let block = client.create_block(compiled(grid.id()));
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::<CompiledLogicApp>::new(editor);
    editor.run();

    assert_eq!(editor.label("compiled-logic.source"), "Half adder");
}
