use std::collections::HashSet;

use super::*;

#[test]
fn a_part_reused_from_the_loaded_head_is_not_sealed_again() {
    let vault = vault(8);
    let parts: Vec<Vec<u8>> = (0..6).map(|seed| pseudorandom(60, seed)).collect();
    fn tree(parts: &[Vec<u8>]) -> Part<'_> {
        Part::Node(vec![
            Part::from(&parts[0]),
            Part::Node(parts[1..4].iter().map(Part::from).collect()),
            Part::Node(parts[4..].iter().map(Part::from).collect()),
        ])
    }
    let first = vault.write(id(8), CONTENT, tree(&parts)).unwrap();
    let stored = vault.load(&first).unwrap();
    assert_eq!(stored.bytes, parts.concat());
    let root = stored.root.clone().unwrap();
    assert_eq!(root.children.len(), 3);
    assert_eq!(root.children[1].children.len(), 3);
    assert_eq!(stored.bytes_of(&root.children[1].children[2]), parts[3]);
    assert_eq!(
        tree(&parts).flatten().unwrap(),
        stored.bytes,
        "a tree's leaves in order are its stored form"
    );

    let mut edited = parts.clone();
    edited[2] = pseudorandom(50, 99);
    let reused = Part::Node(vec![
        Part::Reused(root.children[0].reference),
        Part::Node(vec![
            Part::Reused(root.children[1].children[0].reference),
            Part::from(&edited[2]),
            Part::Reused(root.children[1].children[2].reference),
        ]),
        Part::Reused(root.children[2].reference),
    ]);
    let objects = vault.store().count();
    let second = vault.write(id(8), CONTENT, reused).unwrap();
    assert_eq!(vault.read(&second).unwrap(), edited.concat());
    assert_eq!(
        vault.store().count() - objects,
        3,
        "an edit to one part writes that part and the two nodes above it"
    );
    let before: HashSet<_> = vault.objects(&first).unwrap().into_iter().collect();
    assert_eq!(vault.objects_besides(&second, &before).unwrap().len(), 3);

    let encoded_again = vault.write(id(8), CONTENT, tree(&edited)).unwrap();
    assert_eq!(
        encoded_again, second,
        "reusing a part and encoding it again give the same manifest"
    );
}
