use super::*;

#[test]
fn inline_items_stand_for_their_text_or_an_object_replacement_character() {
    assert_eq!(TextSlot::inline(Some("@ada")).bytes(), 4);
    assert_eq!(TextSlot::inline(None).bytes(), 3);
    assert_eq!(TextSlot::Anchored.bytes(), 0);
}
