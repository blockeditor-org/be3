use super::*;

#[test]
fn remaps_that_do_not_read_say_why_and_store_nothing() {
    let mut editor = editor(RemapState::Off, None);

    apply(
        &mut editor,
        "[[group]]\nname = \"Cairn\"\nrules = [{ on = \"Q\", tap = \"Spcae\" }]\n",
    );

    assert_eq!(content(&editor).root().runs(), None);
    editor.snapshot("remaps_that_do_not_read_say_why_and_store_nothing");
}
