use crate::merkle;

/// The empty tree is a prefix of every tree, but a consistency check from size 0 holds only
/// under the empty tree's own root: a made-up root for size 0 proves nothing, so a server
/// cannot pass a head of size 0 with any root of its choosing.
#[test]
fn the_empty_tree_is_a_prefix_only_under_its_own_root() {
    let mut tree = merkle::Tree::new();
    for index in 0..5u8 {
        tree.push(merkle::leaf_hash(&[index]));
    }
    let root = tree.root();
    assert!(merkle::consistent(0, &merkle::empty_root(), 5, &root, &[]));
    assert!(!merkle::consistent(0, &[7; 32], 5, &root, &[]));
    assert!(!merkle::consistent(0, &[7; 32], 0, &[7; 32], &[]));
}
