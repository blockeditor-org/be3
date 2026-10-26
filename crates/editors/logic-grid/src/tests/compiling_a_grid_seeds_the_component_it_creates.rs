use super::*;
use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Modifiers;
use logic_grid_types::CompiledLogicContent;

#[test]
fn compiling_a_grid_seeds_the_component_it_creates() {
    let mut editor = editor();
    editor.key(Key::Four, Modifiers::NONE);
    editor.run();
    editor.key(Key::One, Modifiers::NONE);
    editor.run();
    let point = canvas_point(&editor, Vec2::new(30.0, 30.0));
    editor.click(point);
    editor.run();

    editor.click("logic-grid.compile");
    editor.run();

    let seeded = editor.seeded();
    let component = seeded
        .iter()
        .find(|seeded| seeded.content_type == CompiledLogicContent::CONTENT_TYPE)
        .expect("compiling seeds the new component's program");
    let program = CompiledLogicContent::decode(&component.bytes)
        .unwrap()
        .root()
        .compiled
        .expect("the seeded component holds a program");
    assert!(program.size().width > 0);
    assert!(program.calls().is_empty());
}
