use super::*;

#[test]
fn templates_of_an_editor_of_no_one_type_name_their_block_type() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    document.editors.0[0].1.edits = Some(Edits::ANY_NAME.into());
    assert_eq!(
        document.into_manifest(),
        Err(ManifestError::TemplateWithoutBlockType)
    );
}
