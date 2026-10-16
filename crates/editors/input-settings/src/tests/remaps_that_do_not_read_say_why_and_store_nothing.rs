use super::*;

#[test]
fn remaps_that_do_not_read_say_why_and_store_nothing() {
    let mut editor = remaps_editor(RemapState::Off);

    type_remaps(
        &mut editor,
        "[[group]]\nname = \"Cairn\"\nrules = [{ on = \"Q\", tap = \"Spcae\" }]\n",
    );

    assert_eq!(content(&editor).root().remaps, None);
    editor.snapshot("remaps_that_do_not_read_say_why_and_store_nothing");
}
