use super::*;

#[test]
fn a_text_fragments_marks_move_with_their_chunks_and_its_embeds_travel_in_refs() {
    let mark = |range: Range<usize>, key: &str, value| TextMark {
        range,
        block: false,
        key: key.to_owned(),
        value,
    };
    let embed = Uuid::from_u128(7);
    let fragment = TextFragment {
        chunks: vec![
            TextChunk {
                bytes: b"bold".to_vec(),
                marks: vec![mark(0..4, "bold", MarkValue::Bool(true))],
            },
            TextChunk {
                bytes: "a \u{fffc}".as_bytes().to_vec(),
                marks: vec![
                    mark(0..1, "link", MarkValue::Text("https://a.test".to_owned())),
                    mark(2..5, "embed", MarkValue::Block { id: embed }),
                ],
            },
        ],
        shape: TextShape::Chars,
        schema: Some("basic".to_owned()),
    };
    let mut item = fragment.encode();
    assert_eq!(item.refs, vec![embed], "the embed's block is a ref");
    assert_eq!(item.decode::<TextFragment>(), Some(fragment.clone()));

    let moved = Uuid::from_u128(8);
    item.refs = vec![moved];
    let decoded = item.decode::<TextFragment>().expect("the refs fit");
    assert_eq!(
        decoded.chunks[1].marks[1].value,
        MarkValue::Block { id: moved },
        "a translated ref moves the embed"
    );
    item.refs.clear();
    assert_eq!(
        item.decode::<TextFragment>(),
        None,
        "refs that do not fit fail"
    );

    let joined = fragment.joined();
    assert_eq!(joined.bytes, "bold\na \u{fffc}".as_bytes());
    assert_eq!(
        joined.marks[1].range,
        5..6,
        "joining moves a later chunk's marks past the break"
    );

    let items = fragment.items(Some("<b>bold</b>".to_owned()));
    let formats: Vec<&str> = items.iter().map(|item| item.format.as_str()).collect();
    assert_eq!(
        formats,
        [formats::TEXT_FRAGMENT, formats::HTML, formats::TEXT],
        "html sits between the fragment and its plain text"
    );
}
