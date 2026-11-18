use crate::merkle::{
    self, Tree, TreeHash, consistent, empty_root, leaf_hash, node_hash, range_root,
};

/// RFC 6962's `MTH`, straight from its definition.
fn reference(leaves: &[TreeHash]) -> TreeHash {
    match leaves.len() {
        0 => empty_root(),
        1 => leaves[0],
        n => {
            let k = 1usize << (usize::BITS - 1 - (n - 1).leading_zeros());
            node_hash(&reference(&leaves[..k]), &reference(&leaves[k..]))
        }
    }
}

fn leaves(count: u64) -> Vec<TreeHash> {
    (0..count)
        .map(|index| leaf_hash(&index.to_be_bytes()))
        .collect()
}

fn tree(leaves: &[TreeHash]) -> Tree {
    let mut tree = Tree::new();
    for leaf in leaves {
        tree.push(*leaf);
    }
    tree
}

/// The tree's root is RFC 6962's for every size; every range of every tree, and every newer
/// head of every older one, verifies with its proof and nothing else, and fails once any hash,
/// leaf or size is changed. A proof is O(log n): a page of a million-leaf log, anywhere in it,
/// ties to the head with a few dozen hashes, and so does a head to the next.
#[test]
fn any_range_and_any_newer_head_of_a_merkle_tree_verify_with_log_n_hashes() {
    assert_eq!(empty_root(), Tree::new().root());
    let all = leaves(40);
    let whole = tree(&all);
    for size in 0..=all.len() as u64 {
        let root = whole.root_at(size);
        assert_eq!(root, reference(&all[..size as usize]), "the root of {size}");
        for from in 0..=size {
            for to in from..=size {
                let proof = whole.range_proof(size, from, to);
                let range = &all[from as usize..to as usize];
                assert_eq!(
                    range_root(size, from, range, &proof),
                    Some(root),
                    "leaves {from}..{to} of {size}"
                );
                if let Some(first) = range.first() {
                    let mut changed = range.to_vec();
                    changed[0] = node_hash(first, first);
                    assert_ne!(range_root(size, from, &changed, &proof), Some(root));
                }
                if let Some(first) = proof.first() {
                    let mut changed = proof.clone();
                    changed[0] = node_hash(first, first);
                    assert_ne!(range_root(size, from, range, &changed), Some(root));
                }
                if to < size && from > 0 {
                    assert_ne!(
                        range_root(size, from - 1, range, &proof),
                        Some(root),
                        "a range verifies only at its own place"
                    );
                }
            }
        }
        for old in 0..=size {
            let proof = whole.consistency_proof(old, size);
            let old_root = whole.root_at(old);
            assert!(
                consistent(old, &old_root, size, &root, &proof),
                "{old} to {size}"
            );
            if old > 0 && old < size {
                let forked = node_hash(&old_root, &old_root);
                assert!(
                    !consistent(old, &forked, size, &root, &proof),
                    "a head the newer one does not extend"
                );
                assert!(!consistent(old, &old_root, size, &forked, &proof));
                let mut changed = proof.clone();
                changed[0] = node_hash(&changed[0], &changed[0]);
                assert!(!consistent(old, &old_root, size, &root, &changed));
                assert!(
                    !consistent(old, &old_root, size, &root, &proof[1..]),
                    "a proof cut short"
                );
            }
        }
    }

    // A log whose first leaves were rewritten is not consistent with its older head.
    let mut rewritten = all.clone();
    rewritten[3] = leaf_hash(b"rewritten");
    let rewritten = tree(&rewritten);
    assert!(!consistent(
        10,
        &whole.root_at(10),
        40,
        &rewritten.root(),
        &rewritten.consistency_proof(10, 40),
    ));

    // Proofs stay logarithmic: a page anywhere in a long log, and a head that grew by one, by
    // half or from one leaf.
    let size = (1u64 << 17) + 3;
    let many = leaves(size);
    let big = tree(&many);
    for (from, to) in [
        (0, 50),
        (size / 2 - 25, size / 2 + 25),
        (7, size - 9),
        (size - 50, size),
    ] {
        let proof = big.range_proof(size, from, to);
        assert!(
            proof.len() <= 2 * 18,
            "{} hashes for {from}..{to}",
            proof.len()
        );
        let range = &many[from as usize..to as usize];
        assert_eq!(range_root(size, from, range, &proof), Some(big.root()));
    }
    for old in [1, size / 2 + 7, size - 1] {
        let proof = big.consistency_proof(old, size);
        assert!(proof.len() <= 2 * 18, "{} hashes from {old}", proof.len());
        assert!(consistent(
            old,
            &big.root_at(old),
            size,
            &big.root(),
            &proof
        ));
    }

    // A frontier appends as the whole tree does, from any size it is read at.
    let mut frontier = merkle::Frontier::default();
    for (index, leaf) in all.iter().enumerate() {
        let made = frontier.push(*leaf);
        assert_eq!(made[0], (0, index as u64, *leaf));
        for (level, at, hash) in made {
            assert_eq!(whole.node(level, at), Ok(hash));
        }
        assert_eq!(frontier.root(), whole.root_at(index as u64 + 1));
        let read = merkle::Frontier::of(index as u64 + 1, &mut |level, at| whole.node(level, at));
        assert_eq!(read.as_ref(), Ok(&frontier));
    }

    // A store's perfect subtrees answer every proof alike.
    let mut node = |level, index| whole.node(level, index);
    assert_eq!(merkle::root(40, &mut node), Ok(whole.root()));
}
