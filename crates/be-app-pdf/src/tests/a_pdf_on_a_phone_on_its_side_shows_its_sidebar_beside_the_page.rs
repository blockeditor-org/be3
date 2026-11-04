use super::*;
use be_sdk_beui::beui::Vec2;
use be_type_pdf::PdfFile;

#[test]
fn a_pdf_on_a_phone_on_its_side_shows_its_sidebar_beside_the_page() {
    let page =
        "0.2 0.4 0.8 rg 20 20 160 50 re f BT /F1 18 Tf 0 0 0 rg 24 100 Td (Hello, PDF) Tj ET";
    let content = PdfFile::from_file("hello.pdf".to_owned(), document(page)).unwrap();
    let mut editor = editor(content).with_size(Vec2::new(844.0, 390.0));

    editor.settle_until("the page to be rendered", |editor| {
        editor.shown("pdf.error") || (editor.shown("pdf.tile.0") && !editor.wants_another_frame())
    });

    assert!(
        editor.shown("pdf.toolbar") && editor.shown("pdf.replace"),
        "a phone shows the page with its toolbar and the sidebar gathered beside it"
    );
    editor.snapshot("a_pdf_on_a_phone_on_its_side_shows_its_sidebar_beside_the_page");
}
