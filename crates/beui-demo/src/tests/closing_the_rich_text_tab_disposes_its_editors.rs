use super::*;

#[test]
fn closing_the_rich_text_tab_disposes_its_editors() {
    let mut test = demo(WIDE);
    open(&mut test, Page::RichText);
    let editor = test.rect_of("demo.rich.editor");
    test.click(pos2(editor.left() + 200.0, editor.top() + 40.0));
    test.frame(Vec::new());
    test.click(format!("dock.tab.{}.close", tab(Page::RichText)));
    test.frame(Vec::new());
    test.frame(Vec::new());
    assert!(!test.shows("demo.rich.editor"), "the tab closes");
}
