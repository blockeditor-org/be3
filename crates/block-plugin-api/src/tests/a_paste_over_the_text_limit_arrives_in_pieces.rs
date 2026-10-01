use super::*;

#[test]
fn a_paste_over_the_text_limit_arrives_in_pieces() {
    let text = "ab€cdé€".repeat(5);
    let pieces = text_pieces(&text, 4);

    assert_eq!(pieces.concat(), text);
    assert!(
        pieces
            .iter()
            .all(|piece| !piece.is_empty() && piece.len() <= 4)
    );
    assert!(text_pieces("", 4).is_empty());
    assert_eq!(
        paste_events("short"),
        vec![InputEvent::Paste("short".to_owned())]
    );
}
