use super::*;

#[test]
fn positions_follow_edits_and_come_back_with_undo() {
    let buffer = TextBuffer::new("0123456789");
    let (early, middle, late) = {
        let read = buffer.read().unwrap();
        (
            read.anchor(2).unwrap(),
            read.anchor(5).unwrap(),
            read.anchor(9).unwrap(),
        )
    };

    buffer.edit(Vec::new(), &mut |edit| edit.replace(4, 3, b"abcdefghij"));
    {
        let read = buffer.read().unwrap();
        assert_eq!(read.anchor_index(early), Some(2));
        assert_eq!(
            read.anchor_index(middle),
            None,
            "the deleted byte took its position"
        );
        assert_eq!(
            read.deleted_anchor_index(middle),
            Some(14),
            "it knows where its byte was deleted"
        );
        assert_eq!(read.anchor_index(late), Some(16));
    }

    buffer.undo();
    let read = buffer.read().unwrap();
    assert_eq!(read.slice(0..read.len()).as_ref(), b"0123456789");
    assert_eq!(
        read.anchor_index(middle),
        Some(5),
        "undoing the delete brings it back"
    );
    assert_eq!(read.deleted_anchor_index(middle), None);
    assert_eq!(read.anchor_index(late), Some(9));
}
