use super::*;

#[test]
fn remaps_written_as_text_are_stored_once_applied() {
    let mut editor = remaps_editor(RemapState::On);

    type_remaps(
        &mut editor,
        "[[group]]\nname = \"Celeste\"\nwindow = { exe = \"Celeste.exe\" }\nrules = [{ on = \"LButton\", send = \"j\" }]\n",
    );

    let groups = content(&editor).root().remap_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Celeste");
    assert_eq!(groups[0].rules.len(), 1);
    editor.snapshot("remaps_written_as_text_are_stored_once_applied");
}
