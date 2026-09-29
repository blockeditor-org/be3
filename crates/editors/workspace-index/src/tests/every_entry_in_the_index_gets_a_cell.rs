use super::*;

#[test]
fn every_entry_in_the_index_gets_a_cell() {
    let (Fixture { mut editor, .. }, children) = editor(3);

    editor.run();

    assert!(!editor.shown("folder.empty"));
    for child in &children {
        assert!(
            editor.shown(&format!("folder.entry.{child}")),
            "the folder has no cell for {child}"
        );
    }
}
