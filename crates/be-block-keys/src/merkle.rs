//! A Merkle tree over an append-only log, as RFC 6962 (Certificate Transparency) keeps one: a
//! log's head is its size and the root hash of the tree over its leaves, so any range of
//! entries is tied to a head by O(log n) hashes (`range_proof`), and a newer head to an older
//! one by O(log n) more (`consistency_proof`). Signed logs (plans/signed_logs.md) use it, and
//! access logs may (plans/deferred/proof_paths_with_inclusion_proofs.md).
//!
//! Hashes are SHA-256 with RFC 6962's prefixes: a leaf is `H(0x00 || data)`, a node
//! `H(0x01 || left || right)`, and the tree over `n` leaves splits at the largest power of two
//! below `n`. Every subtree a proof names is either perfect (2^level leaves starting at a
//! multiple of 2^level), which a store keeps by `(level, index)`, or ends at the tree's right
//! edge, which is made from O(log n) perfect ones (`subtree`). Proofs list their hashes in the
//! order a walk of that split, left before right, meets them, so a verifier walks the same
//! split and takes them in turn.

use sha2::{Digest, Sha256};

/// A hash in the tree: a leaf's, a node's or a root.
pub type TreeHash = [u8; 32];

/// The root of the tree over no leaves: the hash of nothing.
pub fn empty_root() -> TreeHash {
    Sha256::digest([]).into()
}

/// A leaf's hash: `H(0x00 || data)`.
pub fn leaf_hash(data: &[u8]) -> TreeHash {
    let mut hasher = Sha256::new();
    hasher.update([0]);
    hasher.update(data);
    hasher.finalize().into()
}

/// A node's hash: `H(0x01 || left || right)`.
pub fn node_hash(left: &TreeHash, right: &TreeHash) -> TreeHash {
    let mut hasher = Sha256::new();
    hasher.update([1]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

/// Where a tree of `size` leaves (at least two) splits: the largest power of two below it.
fn split(size: u64) -> u64 {
    debug_assert!(size >= 2);
    1 << (63 - (size - 1).leading_zeros())
}

/// Whether `[low, high)` is a perfect subtree: a power of two of leaves, starting at a multiple
/// of it. Its level and index then name it.
fn perfect(low: u64, high: u64) -> Option<(u32, u64)> {
    let width = high - low;
    (width.is_power_of_two() && low % width == 0)
        .then(|| (width.trailing_zeros(), low >> width.trailing_zeros()))
}

/// The hash of the subtree over leaves `[low, high)` of a tree, from the perfect subtrees
/// `node(level, index)` answers: one lookup for a perfect subtree, O(log n) for one ending at
/// the tree's right edge.
pub fn subtree<E>(
    low: u64,
    high: u64,
    node: &mut impl FnMut(u32, u64) -> Result<TreeHash, E>,
) -> Result<TreeHash, E> {
    if high == low {
        return Ok(empty_root());
    }
    if let Some((level, index)) = perfect(low, high) {
        return node(level, index);
    }
    let middle = low + split(high - low);
    Ok(node_hash(
        &subtree(low, middle, node)?,
        &subtree(middle, high, node)?,
    ))
}

/// The root of the tree over a log's first `size` leaves.
pub fn root<E>(
    size: u64,
    node: &mut impl FnMut(u32, u64) -> Result<TreeHash, E>,
) -> Result<TreeHash, E> {
    subtree(0, size, node)
}

/// The perfect subtrees that appending the leaf at `index` completes, itself first at level
/// zero, each as `(level, index, hash)`: what a store keeps so `node` answers them. `node`
/// answers those already kept, of the leaves before it.
pub fn appended<E>(
    index: u64,
    leaf: TreeHash,
    node: &mut impl FnMut(u32, u64) -> Result<TreeHash, E>,
) -> Result<Vec<(u32, u64, TreeHash)>, E> {
    let mut made = vec![(0, index, leaf)];
    let (mut level, mut at, mut hash) = (0u32, index, leaf);
    // A node completes when the subtree it closes ends here: its index is odd at each level.
    while at % 2 == 1 {
        let left = node(level, at - 1)?;
        hash = node_hash(&left, &hash);
        level += 1;
        at /= 2;
        made.push((level, at, hash));
    }
    Ok(made)
}

/// The hashes that tie leaves `[from, to)` to the root of the tree over `size` leaves: the
/// subtrees beside them, in the order a walk of the tree's split meets them. O(log n) for any
/// range, wherever it is in the log.
pub fn range_proof<E>(
    size: u64,
    from: u64,
    to: u64,
    node: &mut impl FnMut(u32, u64) -> Result<TreeHash, E>,
) -> Result<Vec<TreeHash>, E> {
    assert!(from <= to && to <= size, "a range inside the tree");
    let mut proof = Vec::new();
    if size > 0 {
        range_walk(0, size, from, to, node, &mut proof)?;
    }
    Ok(proof)
}

fn range_walk<E>(
    low: u64,
    high: u64,
    from: u64,
    to: u64,
    node: &mut impl FnMut(u32, u64) -> Result<TreeHash, E>,
    proof: &mut Vec<TreeHash>,
) -> Result<(), E> {
    if high <= from || low >= to {
        proof.push(subtree(low, high, node)?);
        return Ok(());
    }
    if from <= low && high <= to {
        return Ok(());
    }
    let middle = low + split(high - low);
    range_walk(low, middle, from, to, node, proof)?;
    range_walk(middle, high, from, to, node, proof)
}

/// The root of the tree over `size` leaves that `leaves`, starting at `from`, and `proof`
/// (`range_proof`) make; none when they do not fit the tree. A root that matches the head's
/// proves the leaves are those of the log at those places.
pub fn range_root(
    size: u64,
    from: u64,
    leaves: &[TreeHash],
    proof: &[TreeHash],
) -> Option<TreeHash> {
    let to = from.checked_add(leaves.len() as u64)?;
    if to > size {
        return None;
    }
    if size == 0 {
        return proof.is_empty().then(empty_root);
    }
    let mut taken = proof.iter();
    let root = range_fold(0, size, from, to, leaves, &mut taken)?;
    taken.next().is_none().then_some(root)
}

fn range_fold(
    low: u64,
    high: u64,
    from: u64,
    to: u64,
    leaves: &[TreeHash],
    proof: &mut std::slice::Iter<'_, TreeHash>,
) -> Option<TreeHash> {
    if high <= from || low >= to {
        return proof.next().copied();
    }
    if high - low == 1 {
        return Some(leaves[(low - from) as usize]);
    }
    let middle = low + split(high - low);
    let left = range_fold(low, middle, from, to, leaves, proof)?;
    let right = range_fold(middle, high, from, to, leaves, proof)?;
    Some(node_hash(&left, &right))
}

/// The hashes that show the tree over `new` leaves extends the tree over its first `old`
/// (RFC 6962's `PROOF(m, D[n])`): empty when `old` is zero or `new`.
pub fn consistency_proof<E>(
    old: u64,
    new: u64,
    node: &mut impl FnMut(u32, u64) -> Result<TreeHash, E>,
) -> Result<Vec<TreeHash>, E> {
    assert!(old <= new, "a newer head is at least as long");
    let mut proof = Vec::new();
    if old > 0 && old < new {
        consistency_walk(old, 0, new, true, node, &mut proof)?;
    }
    Ok(proof)
}

/// RFC 6962's `SUBPROOF(m, D[low:high], whole)`: `old` leaves of `[low, high)` are the older
/// tree's, and `whole` while the older tree is exactly this subtree's left part so far.
fn consistency_walk<E>(
    old: u64,
    low: u64,
    high: u64,
    whole: bool,
    node: &mut impl FnMut(u32, u64) -> Result<TreeHash, E>,
    proof: &mut Vec<TreeHash>,
) -> Result<(), E> {
    if old == high - low {
        if !whole {
            proof.push(subtree(low, high, node)?);
        }
        return Ok(());
    }
    let middle = low + split(high - low);
    if low + old <= middle {
        consistency_walk(old, low, middle, whole, node, proof)?;
        proof.push(subtree(middle, high, node)?);
    } else {
        consistency_walk(old - (middle - low), middle, high, false, node, proof)?;
        proof.push(subtree(low, middle, node)?);
    }
    Ok(())
}

/// Whether `proof` (`consistency_proof`) shows that the tree over `new` leaves with root
/// `new_root` extends the tree over `old` leaves with root `old_root`: a log that was not cut,
/// rewritten or forked between the two heads.
pub fn consistent(
    old: u64,
    old_root: &TreeHash,
    new: u64,
    new_root: &TreeHash,
    proof: &[TreeHash],
) -> bool {
    if old > new {
        return false;
    }
    if old == 0 {
        return proof.is_empty() && (new > 0 || *new_root == empty_root());
    }
    if old == new {
        return proof.is_empty() && old_root == new_root;
    }
    let mut taken = proof.iter();
    let Some((older, newer)) = consistency_fold(old, 0, new, true, old_root, &mut taken) else {
        return false;
    };
    taken.next().is_none() && older == *old_root && newer == *new_root
}

/// Walks `consistency_walk`'s split taking `proof`'s hashes in turn, answering the roots of
/// the older and the newer tree's parts of `[low, high)`.
fn consistency_fold(
    old: u64,
    low: u64,
    high: u64,
    whole: bool,
    old_root: &TreeHash,
    proof: &mut std::slice::Iter<'_, TreeHash>,
) -> Option<(TreeHash, TreeHash)> {
    if old == high - low {
        let hash = match whole {
            true => *old_root,
            false => *proof.next()?,
        };
        return Some((hash, hash));
    }
    let middle = low + split(high - low);
    if low + old <= middle {
        let (older, newer) = consistency_fold(old, low, middle, whole, old_root, proof)?;
        let right = proof.next()?;
        Some((older, node_hash(&newer, right)))
    } else {
        let (older, newer) =
            consistency_fold(old - (middle - low), middle, high, false, old_root, proof)?;
        let left = proof.next()?;
        Some((node_hash(left, &older), node_hash(left, &newer)))
    }
}

/// The right edge of a log's tree: the perfect subtrees its size splits into, largest (leftmost)
/// first, O(log n) of them. Enough to append leaves and know the new root without the rest of
/// the tree: how a store appends a batch, and how a log with no server keeps its head.
#[derive(Clone, Debug, Default, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct Frontier {
    size: u64,
    nodes: Vec<TreeHash>,
}

impl Frontier {
    /// The frontier of a log's first `size` leaves, from the perfect subtrees `node` answers.
    pub fn of<E>(
        size: u64,
        node: &mut impl FnMut(u32, u64) -> Result<TreeHash, E>,
    ) -> Result<Self, E> {
        let mut nodes = Vec::new();
        let mut offset = 0;
        for level in (0..64).rev() {
            if size & (1 << level) != 0 {
                nodes.push(node(level, offset >> level)?);
                offset += 1 << level;
            }
        }
        Ok(Self { size, nodes })
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    /// Appends a leaf by its hash, answering the perfect subtrees it completes as `appended`
    /// does: itself at level zero first.
    pub fn push(&mut self, leaf: TreeHash) -> Vec<(u32, u64, TreeHash)> {
        let index = self.size;
        let mut made = vec![(0, index, leaf)];
        let mut carry = leaf;
        let mut level = 0;
        while self.size & (1 << level) != 0 {
            let left = self
                .nodes
                .pop()
                .expect("a set bit of the size has its subtree");
            carry = node_hash(&left, &carry);
            level += 1;
            made.push((level, index >> level, carry));
        }
        self.nodes.push(carry);
        self.size += 1;
        made
    }

    /// The root of the tree over the leaves so far.
    pub fn root(&self) -> TreeHash {
        let mut nodes = self.nodes.iter().rev();
        let Some(last) = nodes.next() else {
            return empty_root();
        };
        nodes.fold(*last, |right, left| node_hash(left, &right))
    }
}

/// The perfect subtrees of a log's whole tree, kept in memory: a reference for tests, and the
/// tree of a log short enough to hold whole.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    /// Each level's perfect subtrees, leaves at level zero.
    levels: Vec<Vec<TreeHash>>,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> u64 {
        self.levels.first().map_or(0, |leaves| leaves.len() as u64)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Appends a leaf by its hash (`leaf_hash`).
    pub fn push(&mut self, leaf: TreeHash) {
        let index = self.len();
        let made = appended(index, leaf, &mut |level, index| self.node(level, index))
            .expect("a tree in memory holds every node an append asks for");
        for (level, _, hash) in made {
            let level = level as usize;
            if self.levels.len() <= level {
                self.levels.push(Vec::new());
            }
            self.levels[level].push(hash);
        }
    }

    /// A perfect subtree's hash, by its level and index.
    pub fn node(&self, level: u32, index: u64) -> Result<TreeHash, MissingNode> {
        self.levels
            .get(level as usize)
            .and_then(|nodes| nodes.get(index as usize))
            .copied()
            .ok_or(MissingNode { level, index })
    }

    /// The root over the first `size` leaves.
    pub fn root_at(&self, size: u64) -> TreeHash {
        root(size, &mut |level, index| self.node(level, index))
            .expect("a prefix of the tree is in it")
    }

    pub fn root(&self) -> TreeHash {
        self.root_at(self.len())
    }

    /// `range_proof` of leaves `[from, to)` in the tree over the first `size`.
    pub fn range_proof(&self, size: u64, from: u64, to: u64) -> Vec<TreeHash> {
        range_proof(size, from, to, &mut |level, index| self.node(level, index))
            .expect("a prefix of the tree is in it")
    }

    /// `consistency_proof` from the first `old` leaves to the first `new`.
    pub fn consistency_proof(&self, old: u64, new: u64) -> Vec<TreeHash> {
        consistency_proof(old, new, &mut |level, index| self.node(level, index))
            .expect("a prefix of the tree is in it")
    }
}

/// A perfect subtree a store does not hold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingNode {
    pub level: u32,
    pub index: u64,
}
