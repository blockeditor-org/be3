use super::*;

#[test]
fn an_editor_that_edits_nothing_declares_no_templates() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    let editor = &mut document.editors.0[0].1;
    editor.edits = None;
    editor.templates = InOrder(vec![(
        "main".to_owned(),
        TemplateDocument {
            block_type: Some("636f756e-7465-722d-626c-6f636b2d0001".to_owned()),
            ..TemplateDocument::default()
        },
    )]);
    assert_eq!(
        document.into_manifest(),
        Err(ManifestError::ServiceWithTemplates)
    );
}
