use super::*;

#[test]
fn a_copy_adds_its_be3_formats_and_the_image_arboard_did_not_write() {
    let items = [
        ClipItem::new(formats::BLOCKS, b"blocks".to_vec()),
        ClipItem::text("plain"),
        ClipItem::png(b"png".to_vec()),
    ];
    let with_text = beside(&items, true);
    let formats_of = |items: &[&ClipItem]| {
        items
            .iter()
            .map(|item| item.format.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(formats_of(&with_text.be3), [formats::BLOCKS]);
    assert_eq!(
        with_text.png.map(|png| png.format.as_str()),
        Some(formats::PNG),
        "arboard wrote the text, so the image goes beside it"
    );
    let without_text = beside(&items, false);
    assert!(
        without_text.png.is_none(),
        "with no text, arboard wrote the image itself"
    );
}
