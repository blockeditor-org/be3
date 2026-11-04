use super::*;

#[test]
fn a_pdf_checks_what_it_holds_and_round_trips() {
    assert!(PdfFile::from_file("notes.txt", b"hello".to_vec()).is_err());

    let pdf = PdfFile::from_file("paper.pdf", b"%PDF-1.7 body".to_vec()).unwrap();
    let read = PdfContent::decode(&pdf.encode()).unwrap();
    assert_eq!(read, pdf);
    assert_eq!(BlockContent::name(&read).as_deref(), Some("paper.pdf"));
}
