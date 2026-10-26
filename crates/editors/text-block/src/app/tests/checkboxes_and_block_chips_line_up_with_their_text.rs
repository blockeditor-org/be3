use super::{
    BeuiTest, EditorHost, Key, ORGANIZATION_ID, OwnedEditor, TextApp, TextBlock, Uuid, block_url,
};
use block_editor_beui::beui::Modifiers;

#[test]
fn checkboxes_and_block_chips_line_up_with_their_text() {
    let referenced = Uuid::from_u128(0x01d0_0000_0000_4000_8000_0000_0000_0001);
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_organization_id(ORGANIZATION_ID);
    let mut editor = BeuiTest::<TextApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    editor.hold(
        None,
        TextBlock::of(&format!(
            "- [ ] buy milk\n- [x] water the plants\nsee {} here\n",
            block_url("https://example.com/", ORGANIZATION_ID, referenced)
        )),
    );
    editor.run();
    editor.record();

    editor.click("text.surface");
    editor.run();
    for key in [Key::ArrowUp, Key::ArrowUp, Key::Home]
        .into_iter()
        .chain([Key::ArrowRight; 3])
    {
        editor.key(key, Modifiers::NONE);
        editor.run();
    }
    editor.record();

    editor.snapshot("checkboxes_and_block_chips_line_up_with_their_text");
}
