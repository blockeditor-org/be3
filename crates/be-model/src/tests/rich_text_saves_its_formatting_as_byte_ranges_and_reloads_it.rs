use super::*;

#[test]
fn rich_text_saves_its_formatting_as_byte_ranges_and_reloads_it() {
    let document = caption("Title\nsome bold words", vec![heading(0..5), bold(11..15)]);

    let stored = crate::stored::diagnostic(&document.to_stored());
    assert!(
        stored.contains(r#""words": {"text": h'"#),
        "the text is saved as bytes: {stored}"
    );
    assert!(
        stored.contains(r#"{"end": 15, "key": "bold", "start": 11, "value": true}"#),
        "an inline mark is saved as a byte range: {stored}"
    );
    assert!(
        stored.contains(r#"{"end": 5, "key": "heading", "block": true, "start": 0, "value": 1}"#),
        "a line's format is saved as the range of its lines: {stored}"
    );

    let reloaded = Document::<Caption>::from_bytes(&document.to_bytes()).expect("the bytes decode");
    assert_eq!(reloaded, document);
    assert_eq!(seeds(&reloaded), vec![bold(11..15), heading(0..5)]);
    let formats = reloaded
        .rich(ObjectId::ROOT, Caption::WORDS)
        .expect("the caption has words")
        .formats();
    assert_eq!(
        formats.blocks[0].props.get("heading"),
        Some(&MarkValue::Int(1))
    );
    assert!(
        formats
            .marks_at(12)
            .is_some_and(|marks| marks.has("bold", &MarkValue::Bool(true)))
    );
    assert!(formats.marks_at(9).is_some_and(|marks| marks.is_empty()));
}
