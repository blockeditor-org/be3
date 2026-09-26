use super::*;

#[test]
fn manifest_document_defaults_what_the_host_can() {
    let document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    let editor = &document.editors[0];
    assert_eq!(editor.children, ChildOperations::default());
    assert_eq!(editor.interaction, InteractionMode::default());
    assert_eq!(editor.capabilities, EditorCapabilities::default());
    assert_eq!(editor.resize, ResizeMode::default());
    assert!(document.network.is_empty());
    let (_, another) = &editor.templates.0[2];
    assert_eq!(another.category, TemplateCategory::Regular);
}
