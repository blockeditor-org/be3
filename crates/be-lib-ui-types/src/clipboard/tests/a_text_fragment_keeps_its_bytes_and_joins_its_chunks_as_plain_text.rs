use super::*;

#[test]
fn a_text_fragment_keeps_its_bytes_and_joins_its_chunks_as_plain_text() {
    let chars = TextFragment::plain([b"one".to_vec(), vec![b't', 0xff, b'o']], TextShape::Chars);
    let item = chars.encode();
    assert_eq!(item.format, formats::TEXT_FRAGMENT);
    assert_eq!(
        item.decode::<TextFragment>(),
        Some(chars.clone()),
        "bytes that are not UTF-8 survive"
    );
    assert_eq!(
        chars.plain_text(),
        "one\nt\u{fffd}o",
        "chunks join with a line break"
    );

    let lines = TextFragment::plain([b"one\n".to_vec(), b"two\n".to_vec()], TextShape::Lines);
    assert_eq!(
        lines.plain_text(),
        "one\ntwo\n",
        "a chunk that ends a line needs no break after it"
    );

    let plain = ClipItem::text("typed");
    assert_eq!(
        TextFragment::of(&plain),
        Some(TextFragment::chars("typed")),
        "plain text reads as one run of characters"
    );
    assert_eq!(plain.decode::<TextFragment>(), None);
    assert_eq!(
        TextFragment::of(&ClipItem::png(vec![1])),
        None,
        "nor does anything else"
    );
}
