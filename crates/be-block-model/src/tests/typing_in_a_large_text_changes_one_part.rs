use super::*;

#[test]
fn typing_in_a_large_text_changes_one_part() {
    let mut document = note(&prose(7, 200_000));
    let before = document.to_parts();
    assert!(before.children().len() > 10, "the text splits into parts");

    typed_into(&mut document, 600_000, "typed here");

    let parts = document.to_parts();
    assert_eq!(changed_parts(&before, &parts), 1);
    assert_eq!(body(&from_parts::<Note>(&parts)), body(&document));
}
