use super::*;

#[test]
fn anchors_follow_edits_and_die_with_their_bytes() {
    let mut table = AnchorTable::default();
    let early = table.anchor(2);
    let middle = table.anchor(5);
    let late = table.anchor(9);
    assert_eq!(table.anchor(5), middle, "one index has one anchor");

    let removed = table.splice(4, 3, 10);
    assert_eq!(removed, vec![(1, middle)]);
    assert_eq!(table.index(early), Some(2));
    assert_eq!(
        table.index(middle),
        None,
        "the deleted byte took its anchor"
    );
    assert_eq!(
        table.deleted_at(middle),
        Some(14),
        "it remembers where its byte was deleted"
    );
    assert_eq!(table.index(late), Some(16));

    table.splice(4, 10, 3);
    table.restore(4, &removed);
    assert_eq!(
        table.index(middle),
        Some(5),
        "undoing the delete brings it back"
    );
    assert_eq!(table.deleted_at(middle), None);
    assert_eq!(table.index(late), Some(9));

    table.remap(|index| if index == 2 { Err(1) } else { Ok(index + 1) });
    assert_eq!(table.index(early), None);
    assert_eq!(table.deleted_at(early), Some(1));
    assert_eq!(table.index(middle), Some(6));
    assert_eq!(table.len(), 2);
}
