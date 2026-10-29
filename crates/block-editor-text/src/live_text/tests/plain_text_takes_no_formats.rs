use super::*;

#[test]
fn plain_text_takes_no_formats() {
    let content = note("hello");
    let plain = LiveText::new(
        FieldOf::plain(ObjectId::ROOT, Note::PLAIN),
        Waker::default(),
    );
    plain.adopt(&content);

    plain.edit(Vec::new(), &mut |edit| {
        edit.format(0..5, &change(BOLD, Some(Value::Bool(true))));
        edit.replace(5, 0, b"!");
    });

    assert_eq!(plain.bytes(), b"hello!");
    assert_eq!(
        plain.read().map(|read| read.language()),
        Some(text_editor_core::TextLanguage::PlainText)
    );
    assert!(plain.read().and_then(|read| read.formats()).is_none());
    let sent = plain.take_operations();
    assert!(
        sent.iter()
            .flat_map(|edit| &edit.0)
            .all(|change| matches!(change, be_block::be_model::Change::Text { .. })),
        "only the typing is sent: {sent:?}"
    );
}
