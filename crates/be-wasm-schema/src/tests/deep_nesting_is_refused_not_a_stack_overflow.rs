use super::*;

fn chain(depth: usize) -> Tree {
    let mut tree = Tree {
        value: 0,
        children: Vec::new(),
    };
    for value in 1..depth {
        tree = Tree {
            value: value as i32,
            children: vec![tree],
        };
    }
    tree
}

#[test]
fn deep_nesting_is_refused_not_a_stack_overflow() {
    // Each level of a tree is two: its message and its list of children.
    let shallow = chain(MAX_DEPTH as usize / 2);
    assert_eq!(decode::<Tree>(&encode(&shallow)), Ok(shallow));
    let deep = encode(&chain(MAX_DEPTH as usize / 2 + 1));
    assert_eq!(decode::<Tree>(&deep), Err(Error::TooDeep));
    // Bytes nesting far deeper than any value would, written directly: a tree whose children
    // (field 2) are one tree, a thousand times over.
    let mut deeper = Vec::new();
    for _ in 0..1000 {
        let mut tree = Vec::new();
        write_key(Key::new(2), Wire::Len, &mut tree);
        with_length(&mut tree, |list| write_value(deeper.as_slice(), list));
        deeper = tree;
    }
    assert_eq!(decode::<Tree>(&deeper), Err(Error::TooDeep));
}
