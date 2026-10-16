use super::*;

#[test]
fn remaps_written_as_text_name_this_plugins_remapper_once_applied() {
    let mut editor = editor(RemapState::On);

    let text = "# jump with the left button\n[[group]]\nname = \"Celeste\"\nwindow = { exe = \"Celeste.exe\" }\nrules = [{ on = \"LButton\", send = \"j\" }]\n";
    apply(&mut editor, text);

    assert_eq!(
        content(&editor).root().runs(),
        Some(("be3.input-remaps", "remapper.wasm", text.as_bytes())),
        "the text is stored as it was written, for this plugin's remapper to read"
    );
    editor.snapshot("remaps_written_as_text_name_this_plugins_remapper_once_applied");
}
