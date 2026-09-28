use super::*;

#[test]
fn a_page_is_rendered_with_pdfium() {
    let page = "0.2 0.4 0.8 rg 20 20 160 50 re f BT /F1 18 Tf 0 0 0 rg 24 100 Td (Hello, PDF) Tj ET";
    let content = PdfContent::from_file("hello.pdf".to_owned(), document(page)).unwrap();
    let mut editor = editor(content);

    editor.settle_until("the page to be rendered", |editor| {
        editor.shown("pdf.error") || (editor.shown("pdf.tile.0") && !editor.wants_another_frame())
    });

    assert!(!editor.shown("pdf.error"), "{}", editor.label("pdf.error"));
    editor.snapshot("a_page_is_rendered_with_pdfium");
}
