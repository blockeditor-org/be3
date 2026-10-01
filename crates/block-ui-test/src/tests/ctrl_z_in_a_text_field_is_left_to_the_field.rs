use super::*;

use beui::styled::TextInput;
use beui::{Key, Modifiers};
use block_editor_beui::BlockHistory;

#[test]
fn ctrl_z_in_a_text_field_is_left_to_the_field() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ChildApp>::with_view(Editor::new(host, block), || {
        view! {
            <TextInput @test_id={"field"} value=String::new() label="Field" />
        }
    })
    .with_top_bar(false);
    test.set_histories([(
        block,
        BlockHistory {
            can_undo: true,
            can_redo: false,
        },
    )]);
    test.run();
    test.click("field");
    test.run();

    test.key_press_modifiers(Modifiers::CTRL, Key::Z);
    test.run();

    assert!(
        test.take_block_commands().is_empty(),
        "undo in a focused text field undoes the typing, not the block"
    );
}
