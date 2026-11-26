use super::*;

/// A pending edit that sets a card a peer removed before the content was loaded afresh has
/// nothing left to change there: its change is dropped, the rest of the edit carries, and an
/// edit of nothing but such changes carries as none.
#[test]
fn a_change_to_an_object_gone_from_content_loaded_afresh_does_not_carry_over() {
    let mut old = board();
    let (_, _, write) = ids(&old);
    let (ship, insert) = Column::CARDS.insert(ids(&old).0, Anchor::End, &card("ship"));
    old.apply(&insert.into());
    let mut new = Document::<Board>::from_bytes(&old.to_bytes()).expect("the bytes decode");
    new.apply(&Change::remove(ship).into());

    let gone: Edit = Card::TEXT.set(ship, &"shipped".to_owned()).into();
    assert_eq!(Document::carry_over(&gone, &old, &new, 2), None);

    let both = Edit(vec![
        Card::TEXT.set(ship, &"shipped".to_owned()),
        Card::DONE.set(write, &true),
    ]);
    assert_eq!(
        Document::carry_over(&both, &old, &new, 2),
        Some(Edit(vec![Card::DONE.set(write, &true)]))
    );
}
