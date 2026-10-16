use super::*;

#[test]
fn remaps_written_as_text_are_stored_once_applied() {
    let mut editor = remaps_editor(RemapState::On);

    let text = "# jump with the left button\n[[group]]\nname = \"Celeste\"\nwindow = { exe = \"Celeste.exe\" }\nrules = [{ on = \"LButton\", send = \"j\" }]\n";
    type_remaps(&mut editor, text);

    let root = content(&editor).root();
    assert_eq!(
        root.remaps.as_deref(),
        Some(text),
        "the text is stored as it was written"
    );
    let groups = root.remap_groups().expect("the stored text reads");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Celeste");
    assert_eq!(groups[0].rules.len(), 1);
    editor.snapshot("remaps_written_as_text_are_stored_once_applied");
}
