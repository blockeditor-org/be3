use super::*;

#[test]
fn an_editor_is_found_by_its_id() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    let editor = &mut document.editors.0[0].1;
    editor.edits = Some(Edits::ANY_NAME.into());
    editor.templates = InOrder::default();
    let manifest = document
        .into_manifest()
        .expect("any is what an editor may edit");
    let editor = &manifest.editors[0];
    assert_eq!(editor.edits, Edits::Any);
    assert_eq!(editor.name, "counter");
    let found = manifest.editor(editor.id).expect("the editor has this id");
    assert_eq!(found.display_name, "Counter");
    assert!(manifest.editor([9; 16]).is_none());
}
