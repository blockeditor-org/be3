use super::*;

/// A pile's cards share its spot. Only the one on top, which a press there takes, is read
/// and named for it; the rest lie under it, out of the accessibility tree.
#[test]
fn a_pile_is_read_once_as_its_top_card() {
    let editor = editor_after(CRAZY_8S.to_vec(), dealt());
    let document = editor.document();
    let read = document
        .accessibility_view()
        .expect("the editor has an accessibility tree")
        .nodes()
        .filter(|node| {
            node.label()
                .is_some_and(|label| label.starts_with("Card, pile 1"))
        })
        .count();
    assert_eq!(read, 1, "the draw pile is read once, not once a card");
    assert!(document.find_test_id("game.piece.pile.0").is_some());
}
