use super::*;

#[test]
fn rich_text_a_pasted_fragment_never_splits_a_character_or_keeps_an_unsafe_link() {
    let mark = |range: std::ops::Range<usize>, key: &str, value: MarkValue| TextMark {
        range,
        block: false,
        key: key.to_owned(),
        value,
    };
    let fragment = TextFragment {
        chunks: vec![TextChunk {
            bytes: "aéb ok".as_bytes().to_vec(),
            marks: vec![
                mark(2..4, "bold", MarkValue::Bool(true)),
                mark(5..7, schema::basic::LINK, MarkValue::Text("javascript:alert(1)".to_owned())),
                mark(5..7, "italic", MarkValue::Bool(true)),
            ],
        }],
        shape: TextShape::Chars,
        schema: Some("basic".to_owned()),
    };
    let mut tester = EditorTester::rich(b"");
    tester.execute(EditorCommand::Paste(&fragment));
    assert_eq!(
        formatted(&tester),
        "a{bold}\u{c3}\u{a9}b{} {italic}ok",
        "a mark inside a character snaps to its start, and a script link is dropped"
    );
}
