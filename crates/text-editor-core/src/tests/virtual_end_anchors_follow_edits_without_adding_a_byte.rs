use super::*;

#[test]
fn virtual_end_anchors_follow_edits_without_adding_a_byte() {
    for original in [b"".as_slice(), b"ab", b"a\0b"] {
        let buffer = TextBuffer::new(original);
        let (anchor, position) = {
            let read = buffer.read().unwrap();
            let anchor = read.anchor(read.len()).unwrap();
            assert_eq!(
                anchor,
                Pos {
                    client: 0,
                    offset: original.len() as u64
                }
            );
            assert_eq!(read.slice(0..read.len()).as_ref(), original);
            assert_eq!(read.anchor_index(anchor), Some(original.len()));
            assert_eq!(read.deleted_anchor_index(anchor), None);
            assert_eq!(read.anchor(read.len() + 1), None);
            (anchor, Position::at(&*read, usize::MAX))
        };

        buffer.edit(Vec::new(), &mut |edit| edit.replace(0, 0, b"xyz"));
        buffer.finish_history_group();
        {
            let read = buffer.read().unwrap();
            assert_eq!(read.len(), original.len() + 3);
            assert_eq!(read.anchor(read.len()), Some(anchor));
            assert_eq!(read.anchor_index(anchor), Some(read.len()));
            assert_eq!(Position::at(&*read, read.len()), position);
            assert_eq!(position.resolve(&*read), read.len());
        }

        buffer.edit(Vec::new(), &mut |edit| {
            edit.replace(0, edit.document().len(), b"");
        });
        {
            let read = buffer.read().unwrap();
            assert!(read.is_empty());
            assert_eq!(read.anchor(0), Some(anchor));
            assert_eq!(read.anchor_index(anchor), Some(0));
            assert_eq!(Position::at(&*read, 0), position);
            assert_eq!(position.resolve(&*read), 0);
        }
        buffer.undo().unwrap();
        {
            let read = buffer.read().unwrap();
            assert_eq!(position.resolve(&*read), original.len() + 3);
        }
        buffer.undo().unwrap();
        {
            let read = buffer.read().unwrap();
            assert_eq!(read.slice(0..read.len()).as_ref(), original);
            assert_eq!(read.anchor(read.len()), Some(anchor));
            assert_eq!(position.resolve(&*read), original.len());
        }
        buffer.redo().unwrap();
        assert_eq!(
            position.resolve(&*buffer.read().unwrap()),
            original.len() + 3
        );
    }
}
