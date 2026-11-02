use super::*;
use be_sdk_beui::beui::Vec2;
use be_type_pdf::PdfFile;

#[test]
fn a_pdf_on_a_phone_shows_its_sidebar_below_the_page() {
    let page =
        "0.2 0.4 0.8 rg 20 20 160 50 re f BT /F1 18 Tf 0 0 0 rg 24 100 Td (Hello, PDF) Tj ET";
    let content = PdfFile::from_file("hello.pdf".to_owned(), document(page)).unwrap();
    let mut editor = editor(content).with_size(Vec2::new(390.0, 844.0));
    let rendered = |editor: &BeuiTest<PdfRoot>| {
        editor.shown("pdf.error") || (editor.shown("pdf.tile.0") && !editor.wants_another_frame())
    };

    editor.settle_until("the page to be rendered", rendered);
    assert!(
        editor.shown("pdf.toolbar") && editor.shown("pdf.replace"),
        "a phone shows the page with its toolbar and the sidebar gathered below it"
    );
    editor.record();

    let splitter = editor.rect_of("dock.compact.splitter").center();
    editor.drag_along(&[
        splitter,
        splitter - Vec2::new(0.0, 100.0),
        splitter - Vec2::new(0.0, 250.0),
    ]);
    editor.run();
    editor.settle_until("the page to be rendered again", rendered);
    let page = editor.rect_of("pdf.tile.0");
    let gathered = editor.rect_of("dock.compact");
    assert!(
        page.top() >= 0.0 && page.bottom() <= gathered.top(),
        "the page stays fitted in the stage the splitter shrank, {page:?} {gathered:?}"
    );
    editor.record();
    editor.snapshot("a_pdf_on_a_phone_shows_its_sidebar_below_the_page");
}
