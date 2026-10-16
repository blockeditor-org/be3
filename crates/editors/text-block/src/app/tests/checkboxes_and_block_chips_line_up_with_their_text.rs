use super::{
    BeuiTest, EditorHost, Key, OwnedEditor, TextApp, TextBlock, Uuid, WORKSPACE_ID, block_url,
};

#[test]
fn checkboxes_and_block_chips_line_up_with_their_text() {
    let referenced = Uuid::from_u128(0x01d0_0000_0000_4000_8000_0000_0000_0001);
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_workspace_id(WORKSPACE_ID);
    let mut editor = BeuiTest::<TextApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    editor.hold(
        None,
        TextBlock::of(&format!(
            "- [ ] buy milk\n- [x] water the plants\nsee {} here\n",
            block_url("https://example.com/", WORKSPACE_ID, referenced)
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
        editor.key_press(key);
        editor.run();
    }
    editor.record();

    editor.snapshot("checkboxes_and_block_chips_line_up_with_their_text");
}
