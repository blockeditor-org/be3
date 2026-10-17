use super::*;

#[test]
fn positions_follow_their_byte_through_replacement_and_undo() {
    for atomic in [false, true] {
        let buffer = TextBuffer::new("abc");
        let position = Position::at(&*buffer.read().unwrap(), 1);
        buffer.edit(Vec::new(), &mut |edit| {
            if atomic {
                edit.replace_atomically(1, 1, b"xyz");
            } else {
                edit.replace(1, 1, b"xyz");
            }
        });
        let expected = if atomic { 1 } else { 4 };
        {
            let read = buffer.read().unwrap();
            assert_eq!(read.slice(0..read.len()).as_ref(), b"axyzc");
            assert_eq!(position.resolve(&*read), expected);
        }
        buffer.undo().unwrap();
        assert_eq!(position.resolve(&*buffer.read().unwrap()), 1);
        buffer.redo().unwrap();
        assert_eq!(position.resolve(&*buffer.read().unwrap()), expected);
    }
}
