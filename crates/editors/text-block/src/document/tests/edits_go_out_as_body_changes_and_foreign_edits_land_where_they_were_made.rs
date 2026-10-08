use super::*;

#[test]
fn edits_go_out_as_body_changes_and_foreign_edits_land_where_they_were_made() {
    let document = BlockDocument::new(Waker::default());
    let mut content = TextBlock::of("hello world");
    document.adopt(&content);

    document.edit(Vec::new(), &mut |edit| edit.replace(5, 0, b" big"));
    for edit in document.take_operations() {
        content.apply(&edit);
    }
    assert_eq!(TextBlock::text(&content), "hello big world");

    let revision = document.revision();
    let foreign = TextBlock::insert(&content, 9, 0, b">> ").expect("there is somewhere to type");
    content.apply(&foreign);
    assert!(document.apply(&foreign));

    assert_eq!(document.bytes(), b">> hello big world");
    assert!(document.take_external_edit());
    assert_eq!(
        document.changes_since(revision),
        Some(TextChange {
            start: 0,
            old_end: 0,
            new_end: 3,
        })
    );
    assert!(
        !document.apply(&TextBlock::set_language(
            block_editor_beui::be_block::TextLanguage::Rust
        )),
        "a header change is left to a whole adoption"
    );
}
