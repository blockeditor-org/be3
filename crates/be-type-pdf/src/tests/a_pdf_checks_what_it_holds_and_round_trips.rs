use super::*;

#[test]
fn a_pdf_checks_what_it_holds_and_round_trips() {
    assert!(!PdfFile::claims("notes.txt", b"hello"));
    assert!(
        !PdfFile::claims("notes.pdf", b"hello"),
        "a .pdf that is not a PDF is not one"
    );
    assert!(PdfFile::claims("paper.pdf", b"%PDF-1.7 body"));

    let pdf = PdfFile::from_file("paper.pdf", b"%PDF-1.7 body".to_vec());
    let read = PdfContent::decode(&pdf.encode()).unwrap();
    assert_eq!(read, pdf);
    assert_eq!(BlockContent::name(&read).as_deref(), Some("paper.pdf"));
}
