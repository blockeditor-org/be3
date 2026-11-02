use super::*;

#[test]
fn an_empty_folder_says_so() {
    let (Fixture { mut editor, .. }, _) = editor(0);

    editor.run();

    assert!(editor.shown("folder.empty"));
}
