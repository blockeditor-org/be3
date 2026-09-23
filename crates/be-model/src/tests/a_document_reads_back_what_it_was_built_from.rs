use super::*;

#[test]
fn a_document_reads_back_what_it_was_built_from() {
    let document = board();

    let decoded = Document::<Board>::from_bytes(&document.to_bytes()).expect("the bytes decode");

    assert_eq!(decoded, document);
    assert_eq!(decoded.root().title, "Plan");
    assert_eq!(
        columns(&decoded),
        owned(&[("Todo", &["write", "review"]), ("Done", &[])])
    );
    assert_eq!(Document::<Board>::default().root(), Board::default());
    assert!(Document::<Board>::from_bytes(&[0]).is_err());
}
