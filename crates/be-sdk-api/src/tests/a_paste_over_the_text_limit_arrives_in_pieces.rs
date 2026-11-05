use super::*;

#[test]
fn a_paste_over_the_text_limit_arrives_in_pieces() {
    let text = "ab€cdé€".repeat(5);
    let pieces = paste_pieces(ClipItem::text(text.clone()), 4);

    let mut joined = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let (Input::PastePiece(item) | Input::Paste(item)) = piece else {
            panic!("not a paste: {piece:?}");
        };
        assert_eq!(matches!(piece, Input::Paste(_)), index + 1 == pieces.len());
        assert!(item.data.len() <= 4);
        joined.extend_from_slice(&item.data);
    }
    assert_eq!(joined, text.as_bytes());
    assert_eq!(
        paste_events(ClipItem::text("short")),
        vec![Input::Paste(ClipItem::text("short"))]
    );
}
