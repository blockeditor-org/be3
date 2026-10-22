use super::*;

#[test]
fn an_editor_that_names_no_block_type_edits_nothing() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    let editor = &mut document.editors.0[0].1;
    editor.edits = None;
    editor.templates = InOrder::default();
    let manifest = document
        .into_manifest()
        .expect("an editor may edit nothing");
    assert_eq!(manifest.editors[0].edits, Edits::Nothing);
}
