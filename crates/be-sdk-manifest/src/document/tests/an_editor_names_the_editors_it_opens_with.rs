use super::*;

#[test]
fn an_editor_names_the_editors_it_opens_with() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    let files = "9026bab5-b23e-4b81-a693-d3a250905f9e";
    document.editors.0[0].1.opens = vec![files.to_owned()];
    let manifest = document
        .clone()
        .into_manifest()
        .expect("the document is valid");
    assert_eq!(
        manifest.editors[0].opens,
        vec![Uuid::parse_str(files).expect("a uuid").into_bytes()]
    );

    document.editors.0[0].1.opens = vec!["files".to_owned()];
    assert_eq!(
        document.into_manifest(),
        Err(ManifestError::InvalidEditorId)
    );
}
