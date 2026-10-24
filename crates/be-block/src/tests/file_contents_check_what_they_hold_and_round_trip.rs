use crate::{AudioContent, BlockContent, PdfContent};

#[test]
fn file_contents_check_what_they_hold_and_round_trip() {
    assert!(PdfContent::from_file("notes.txt", b"hello".to_vec()).is_err());
    assert!(AudioContent::from_file("silence.wav", "audio/wav", Vec::new()).is_err());

    let pdf = PdfContent::from_file("paper.pdf", b"%PDF-1.7 body".to_vec()).unwrap();
    let read = PdfContent::decode(&pdf.encode()).unwrap();
    assert_eq!(read, pdf);
    assert_eq!(BlockContent::name(&read).as_deref(), Some("paper.pdf"));
}
